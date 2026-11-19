//! Background discovery and status reads. Results arrive on the state's channel.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use repose_core::config::Config;
use repose_core::discovery::discover;
use repose_core::git::read_status_into;
use repose_core::model::RepoSummary;

use super::{AppState, Msg, find};

impl AppState {
	/// Walks the roots on a thread, then reads every status through a pool of eight workers.
	/// Origins' visibility is kept: only Check visibility reads it again (remote-visibility).
	pub fn start_scan(&mut self, ctx: &egui::Context) {
		self.generation += 1;
		self.scanning = true;
		self.remotes.clear();
		self.remote_reads.clear();
		self.missing_roots = repose_core::config::missing_roots(&self.config);
		start(
			self.generation,
			self.config.clone(),
			self.tx.clone(),
			ctx.clone(),
		);
	}

	/// Re-reads one repository's status after a write, without rescanning the roots.
	pub fn refresh_repo(&self, path: &Path, ctx: &egui::Context) {
		if let Some(repo) = find(&self.repos, path) {
			refresh_one(self.generation, repo.clone(), self.tx.clone(), ctx.clone());
		}
	}
}

/// Every row, worktrees included, in list order.
fn rows_of(repos: &[RepoSummary]) -> VecDeque<RepoSummary> {
	let mut queue = VecDeque::new();
	for r in repos {
		queue.push_back(r.clone());
		for w in &r.worktrees {
			queue.push_back(w.clone());
		}
	}
	queue
}

/// Reads every queued status through eight workers and returns when the queue is empty.
fn read_all(generation: u64, queue: VecDeque<RepoSummary>, tx: &Sender<Msg>, ctx: &egui::Context) {
	let queue = Arc::new(Mutex::new(queue));
	let workers: Vec<_> = (0..8)
		.map(|_| {
			let queue = Arc::clone(&queue);
			let tx = tx.clone();
			let ctx = ctx.clone();
			std::thread::spawn(move || {
				loop {
					let next = queue.lock().ok().and_then(|mut q| q.pop_front());
					let Some(mut repo) = next else { break };
					repo.worktrees.clear();
					read_status_into(&mut repo);
					let _ = tx.send(Msg::Status(generation, Box::new(repo)));
					ctx.request_repaint();
				}
			})
		})
		.collect();
	for w in workers {
		let _ = w.join();
	}
}

/// How long the rows wait for the clock offset after the walk: a hung `w32tm` holds back only the
/// clocks, never the list (Morticia, note 2 on 3e89430).
const OFFSET_WAIT: Duration = Duration::from_secs(1);

/// Walks the roots, sends the rows, then reads every status through eight workers. The clock
/// offset is read beside the walk (about 60 ms against the walk's hundreds) and sent before the
/// rows, so Activity never counts days from UTC once it has rows to count
/// (remediation-20261116-activity-days-before-the-first-scan).
pub fn start(generation: u64, config: Config, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let (offset_tx, offset_rx) = channel();
		std::thread::spawn(move || offset_tx.send(crate::platform::utc_offset_minutes()));
		let repos = discover(&config);
		let offset = offset_rx.recv_timeout(OFFSET_WAIT).unwrap_or(None);
		let _ = tx.send(Msg::Offset(generation, offset));
		let queue = rows_of(&repos);
		let _ = tx.send(Msg::Discovered(generation, repos));
		ctx.request_repaint();
		read_all(generation, queue, &tx, &ctx);
		// The clock offset is read here, off the UI thread, once per scan.
		let offset = crate::platform::utc_offset_minutes();
		let _ = tx.send(Msg::Done(generation, offset));
		ctx.request_repaint();
	});
}

/// Re-reads every known row's status without walking the roots again: the 60 second backstop
/// behind the file-system watcher, and the answer to a watcher overflow.
pub fn refresh_all(generation: u64, repos: &[RepoSummary], tx: Sender<Msg>, ctx: egui::Context) {
	let queue = rows_of(repos);
	std::thread::spawn(move || {
		read_all(generation, queue, &tx, &ctx);
		let offset = crate::platform::utc_offset_minutes();
		let _ = tx.send(Msg::PassDone(generation, offset));
		ctx.request_repaint();
	});
}

/// Re-reads one repository's status, for example after a write.
pub fn refresh_one(generation: u64, mut repo: RepoSummary, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		repo.worktrees.clear();
		read_status_into(&mut repo);
		let _ = tx.send(Msg::Status(generation, Box::new(repo)));
		ctx.request_repaint();
	});
}

#[cfg(test)]
mod tests {
	use std::sync::mpsc::channel;
	use std::time::Duration;

	use eframe::egui;
	use repose_core::config::Config;

	use super::start;
	use crate::state::{AppState, Msg};

	/// remediation-20261116-activity-days-before-the-first-scan clause 1: a scan sends the clock
	/// offset before its rows, so the offset is in the state before Activity has a row to count.
	#[test]
	fn a_scan_sends_the_offset_before_its_rows() {
		let (tx, rx) = channel();
		start(1, Config::default(), tx, egui::Context::default());
		let first = rx.recv_timeout(Duration::from_secs(20));
		assert!(
			matches!(first, Ok(Msg::Offset(1, _))),
			"the offset comes first"
		);
		let second = rx.recv_timeout(Duration::from_secs(20));
		assert!(matches!(second, Ok(Msg::Discovered(1, _))), "then the rows");
	}

	/// Roger 5Q-1 on 3e89430: an older scan's offset, arriving after a newer scan, is dropped like
	/// its other messages; and a failed read keeps the offset already known (Morticia, note 1).
	#[test]
	fn a_stale_or_failed_offset_never_replaces_a_known_one() {
		let ctx = egui::Context::default();
		let mut state = AppState::new(Config::default());
		state.generation = 2;
		state.utc_offset = Some(-360);
		state.scanning = false;
		let tx = state.sender();
		let deliver = |state: &mut AppState, msg| {
			let _ = tx.send(msg);
			state.pump(&ctx);
		};
		deliver(&mut state, Msg::Offset(1, None));
		assert_eq!(state.utc_offset, Some(-360), "a stale None is dropped");
		deliver(&mut state, Msg::Offset(1, Some(-480)));
		assert_eq!(state.utc_offset, Some(-360), "a stale value is dropped");
		deliver(&mut state, Msg::Offset(2, None));
		assert_eq!(state.utc_offset, Some(-360), "a failed read keeps it");
		deliver(&mut state, Msg::Offset(2, Some(-300)));
		assert_eq!(state.utc_offset, Some(-300), "a current read updates it");
	}
}
