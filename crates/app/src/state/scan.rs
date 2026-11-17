//! Background discovery and status reads. Results arrive on the state's channel.

use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use eframe::egui;
use repose_core::config::Config;
use repose_core::discovery::discover;
use repose_core::git::read_status_into;
use repose_core::model::RepoSummary;

use super::Msg;

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

/// Walks the roots, sends the rows, then reads every status through eight workers. The clock
/// offset is read beside the walk (about 60 ms against the walk's hundreds) and sent before the
/// rows, so Activity never counts days from UTC once it has rows to count
/// (remediation-20261116-activity-days-before-the-first-scan).
pub fn start(generation: u64, config: Config, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let offset = std::thread::spawn(crate::platform::utc_offset_minutes);
		let repos = discover(&config);
		let _ = tx.send(Msg::Offset(offset.join().unwrap_or(None)));
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
	use crate::state::Msg;

	/// remediation-20261116-activity-days-before-the-first-scan clause 1: a scan sends the clock
	/// offset before its rows, so the offset is in the state before Activity has a row to count.
	#[test]
	fn a_scan_sends_the_offset_before_its_rows() {
		let (tx, rx) = channel();
		start(1, Config::default(), tx, egui::Context::default());
		let first = rx.recv_timeout(Duration::from_secs(20));
		assert!(
			matches!(first, Ok(Msg::Offset(_))),
			"the offset comes first"
		);
		let second = rx.recv_timeout(Duration::from_secs(20));
		assert!(matches!(second, Ok(Msg::Discovered(1, _))), "then the rows");
	}
}
