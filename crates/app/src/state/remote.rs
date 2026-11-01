//! The origin URL per repository, read on a thread the first time a repository is selected or a
//! row's menu opens, and kept until the next Refresh (a remote can be added from a terminal).
//! A read that fails is kept as the failure, so the menu can say why the remote items are off
//! rather than claiming there is no origin (Morticia L-3 on 85ec3b0). Reads carry the scan
//! generation, so one that lands after a Refresh is dropped instead of refilling the cleared
//! cache (Roger 5Q-3 on 85ec3b0).

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::remote::read_origin_url;

use super::{AppState, Msg};

/// What is known about a repository's origin: a URL, none, or why it could not be read.
pub type Remote = Result<Option<String>, String>;

impl AppState {
	/// Starts a read when nothing is known for the repository and none is in flight.
	pub fn ensure_remote(&mut self, repo: &Path, ctx: &egui::Context) {
		if self.remotes.contains_key(repo) || self.remote_reads.contains(repo) {
			return;
		}
		self.remote_reads.insert(repo.to_path_buf());
		spawn_remote(
			self.generation,
			repo.to_path_buf(),
			self.tx.clone(),
			ctx.clone(),
		);
	}

	pub(super) fn merge_remote(&mut self, generation: u64, repo: PathBuf, remote: Remote) {
		if generation != self.generation {
			return;
		}
		self.remote_reads.remove(&repo);
		self.remotes.insert(repo, remote);
	}
}

fn spawn_remote(generation: u64, repo: PathBuf, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let remote = read_origin_url(&repo).map_err(|e| e.to_string());
		let _ = tx.send(Msg::Remote(generation, repo, remote));
		ctx.request_repaint();
	});
}
