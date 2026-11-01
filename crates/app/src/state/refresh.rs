//! Live refresh: the file-system watcher is started after the first discovery and told about
//! new rows after every later one; its refreshes re-read one repository's status in place, an
//! overflow re-reads every row once and says so in the status line, and the 60 second backstop
//! covers whatever the watcher missed.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use eframe::egui;
use repose_core::watch::RepoMap;
use repose_core::watch::watcher::{Refresh, Watch};

use super::{AppState, scan};

/// Two overflows inside this window re-read everything once, not twice.
const OVERFLOW_QUIET: Duration = Duration::from_secs(2);

pub struct Watching {
	pub watch: Option<Watch>,
	pub tx: Sender<Refresh>,
	pub rx: Receiver<Refresh>,
	/// A lasting condition for the status line: a root that could not be watched, or no watcher.
	pub note: Option<String>,
	/// An overflow pass finished: every row was re-read after events were lost. Shown until the
	/// next pass or scan completes.
	pub caught_up: bool,
	/// The watcher reported lost events and no pass has covered them yet. Kept until a pass
	/// starts after the report, so an overflow during a pass or a scan is never dropped (Roger
	/// 5Q-1 on 7e70f1f).
	pub full_pending: bool,
	/// The pass running now is the answer to an overflow.
	pub overflow_pass: bool,
	last_overflow: Option<Instant>,
}

impl Default for Watching {
	fn default() -> Self {
		let (tx, rx) = channel();
		Self {
			watch: None,
			tx,
			rx,
			note: None,
			caught_up: false,
			full_pending: false,
			overflow_pass: false,
			last_overflow: None,
		}
	}
}

impl AppState {
	/// After a discovery: start the watcher on the configured roots, or give a running one the
	/// new rows. Roots that cannot be watched are named once in the status line.
	pub(super) fn sync_watch(&mut self, ctx: &egui::Context) {
		let map = RepoMap::new(&self.repos);
		if let Some(w) = &self.watching.watch {
			w.set_map(map);
			return;
		}
		let wake_ctx = ctx.clone();
		let started = Watch::start(
			&self.config.roots,
			self.config.exclude.clone(),
			map,
			self.watching.tx.clone(),
			Box::new(move || wake_ctx.request_repaint()),
		);
		match started {
			Ok((watch, unwatched)) => {
				self.watching.watch = Some(watch);
				if let Some(first) = unwatched.first() {
					self.watching.note = Some(format!(
						"Not watching {}{}: {}",
						first.root.display(),
						if unwatched.len() > 1 {
							format!(" and {} more", unwatched.len() - 1)
						} else {
							String::new()
						},
						first.reason
					));
				}
			}
			Err(e) => {
				self.watching.note =
					Some(format!("No file watcher ({e}); the 60 s timer refreshes"));
			}
		}
	}

	/// Drains the watcher's refreshes: one repository each, and a lost-events report that marks a
	/// full re-read pending. The single repositories are refreshed even when a full re-read is
	/// pending, since that one may have to wait for a running pass.
	pub(super) fn pump_watch(&mut self, ctx: &egui::Context) {
		let mut repos = Vec::new();
		while let Ok(r) = self.watching.rx.try_recv() {
			match r {
				Refresh::Repo(p) => repos.push(p),
				Refresh::All => self.watching.full_pending = true,
			}
		}
		repos.sort();
		repos.dedup();
		for p in repos {
			self.refresh_repo(&p, ctx);
		}
		self.start_full_reread_if_due(ctx);
	}

	/// Starts the pending full re-read when nothing else is reading and the last one started at
	/// least `OVERFLOW_QUIET` ago; otherwise it stays pending and is tried again at the next frame
	/// and at the next pass completion. Returns whether a pass started.
	pub(super) fn start_full_reread_if_due(&mut self, ctx: &egui::Context) -> bool {
		if !self.watching.full_pending {
			return false;
		}
		let now = Instant::now();
		let recent = self
			.watching
			.last_overflow
			.is_some_and(|t| now.duration_since(t) < OVERFLOW_QUIET);
		if self.scanning || self.backstop_running || recent || self.repos.is_empty() {
			// Still pending: an idle window would otherwise not look again until its next
			// repaint, up to the backstop away (Morticia Info on 740b7fd).
			ctx.request_repaint_after(OVERFLOW_QUIET);
			return false;
		}
		self.watching.full_pending = false;
		self.watching.last_overflow = Some(now);
		self.watching.overflow_pass = true;
		// The same single pass as the backstop; pass_done clears the flag and shows the note.
		self.backstop_running = true;
		scan::refresh_all(self.generation, &self.repos, self.tx.clone(), ctx.clone());
		true
	}
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use repose_core::config::Config;
	use repose_core::model::{RepoKind, RepoSummary};
	use repose_core::watch::Refresh;

	use crate::state::AppState;

	/// Roger 5Q-1 on 7e70f1f: a lost-events report that arrives while a scan or a pass is
	/// running must not be dropped; it waits and runs when the pass has finished.
	#[test]
	fn an_overflow_during_a_pass_waits_and_then_runs_once() {
		let ctx = eframe::egui::Context::default();
		let mut state = AppState::new(Config::default());
		state.repos = vec![RepoSummary::new(
			PathBuf::from("D:/t/a"),
			RepoKind::Repository,
		)];
		state.backstop_running = true;
		state
			.watching
			.tx
			.send(Refresh::All)
			.unwrap_or_else(|e| panic!("{e}"));
		state.pump_watch(&ctx);
		assert!(state.watching.full_pending, "kept while a pass runs");
		assert!(!state.watching.overflow_pass);
		// The pass finishes: the pending re-read starts now, as an overflow pass.
		state.pass_done(None, &ctx);
		assert!(!state.watching.full_pending);
		assert!(state.watching.overflow_pass);
		assert!(state.backstop_running);
		assert!(
			!state.watching.caught_up,
			"not claimed before the pass completes"
		);
		// A second report inside the quiet window stays pending rather than being lost.
		state
			.watching
			.tx
			.send(Refresh::All)
			.unwrap_or_else(|e| panic!("{e}"));
		state.pump_watch(&ctx);
		assert!(state.watching.full_pending);
		// That pass completes: Watcher caught up shows, and the second report waits for quiet.
		state.pass_done(None, &ctx);
		assert!(state.watching.caught_up);
		assert!(!state.watching.overflow_pass);
		assert!(
			state.watching.full_pending,
			"inside OVERFLOW_QUIET it waits"
		);
	}
}
