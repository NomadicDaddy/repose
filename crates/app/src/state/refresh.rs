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
	/// Events were lost and every row was re-read; shown until the next full scan completes.
	pub caught_up: bool,
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

	/// Drains the watcher's refreshes: one repository each, or everything after an overflow.
	pub(super) fn pump_watch(&mut self, ctx: &egui::Context) {
		let mut repos = Vec::new();
		let mut all = false;
		while let Ok(r) = self.watching.rx.try_recv() {
			match r {
				Refresh::Repo(p) => repos.push(p),
				Refresh::All => all = true,
			}
		}
		if all {
			let now = Instant::now();
			let recent = self
				.watching
				.last_overflow
				.is_some_and(|t| now.duration_since(t) < OVERFLOW_QUIET);
			if !recent && !self.scanning && !self.backstop_running {
				self.watching.last_overflow = Some(now);
				self.watching.caught_up = true;
				// The same single pass as the backstop; PassDone clears the flag.
				self.backstop_running = true;
				scan::refresh_all(self.generation, &self.repos, self.tx.clone(), ctx.clone());
			}
			return;
		}
		repos.sort();
		repos.dedup();
		for p in repos {
			self.refresh_repo(&p, ctx);
		}
	}
}
