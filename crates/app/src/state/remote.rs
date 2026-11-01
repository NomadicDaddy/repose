//! The origin URL per repository, read on a thread the first time a repository is selected or a
//! row's menu opens, and kept until the next Refresh (a remote can be added from a terminal).
//! A read that fails counts as no origin: the two remote items stay disabled rather than the
//! app guessing a URL.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::remote::read_origin_url;

use super::{AppState, Msg};

impl AppState {
	/// Starts a read when nothing is known for the repository and none is in flight.
	pub fn ensure_remote(&mut self, repo: &Path, ctx: &egui::Context) {
		if self.remotes.contains_key(repo) || self.remote_reads.contains(repo) {
			return;
		}
		self.remote_reads.insert(repo.to_path_buf());
		spawn_remote(repo.to_path_buf(), self.tx.clone(), ctx.clone());
	}

	pub(super) fn merge_remote(&mut self, repo: PathBuf, url: Option<String>) {
		self.remote_reads.remove(&repo);
		self.remotes.insert(repo, url);
	}
}

fn spawn_remote(repo: PathBuf, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let url = read_origin_url(&repo).ok().flatten();
		let _ = tx.send(Msg::Remote(repo, url));
		ctx.request_repaint();
	});
}
