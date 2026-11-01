//! The History pane's state: pages of commits per repository, the expanded commit and its files,
//! loaded on demand on background threads.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::log::{PAGE, read_commit_files, read_log};
use repose_core::model::{Commit, CommitFile};

use super::{AppState, Msg};

#[derive(Default)]
pub struct HistoryState {
	pub commits: Vec<Commit>,
	pub loading: bool,
	pub exhausted: bool,
	pub error: Option<String>,
	pub expanded: Option<String>,
	pub files: HashMap<String, Vec<CommitFile>>,
}

pub enum HistoryMsg {
	Page {
		repo: PathBuf,
		skip: usize,
		result: Result<Vec<Commit>, String>,
	},
	Files {
		repo: PathBuf,
		hash: String,
		result: Result<Vec<CommitFile>, String>,
	},
}

impl AppState {
	pub fn history_of(&self, repo: &Path) -> Option<&HistoryState> {
		self.history.get(repo)
	}

	/// Loads the first page when the pane first shows a repository.
	pub fn ensure_history(&mut self, repo: &Path, ctx: &egui::Context) {
		if self.history.contains_key(repo) {
			return;
		}
		self.history
			.insert(repo.to_path_buf(), HistoryState::default());
		self.load_history_page(repo, 0, ctx);
	}

	pub fn load_more_history(&mut self, repo: &Path, ctx: &egui::Context) {
		let skip = self.history.get(repo).map(|h| h.commits.len()).unwrap_or(0);
		self.load_history_page(repo, skip, ctx);
	}

	/// Drops a repository's history so the next view reloads it, after a commit.
	pub fn invalidate_history(&mut self, repo: &Path) {
		self.history.remove(repo);
	}

	fn load_history_page(&mut self, repo: &Path, skip: usize, ctx: &egui::Context) {
		if let Some(h) = self.history.get_mut(repo) {
			if h.loading || h.exhausted {
				return;
			}
			h.loading = true;
		}
		spawn_page(repo.to_path_buf(), skip, self.tx.clone(), ctx.clone());
	}

	/// Expands one commit (collapsing any other) and fetches its files once.
	pub fn toggle_commit(&mut self, repo: &Path, hash: &str, ctx: &egui::Context) {
		let Some(h) = self.history.get_mut(repo) else {
			return;
		};
		if h.expanded.as_deref() == Some(hash) {
			h.expanded = None;
			return;
		}
		h.expanded = Some(hash.to_string());
		if !h.files.contains_key(hash) {
			spawn_files(
				repo.to_path_buf(),
				hash.to_string(),
				self.tx.clone(),
				ctx.clone(),
			);
		}
	}

	pub(super) fn merge_history(&mut self, msg: HistoryMsg) {
		match msg {
			HistoryMsg::Page { repo, skip, result } => {
				let Some(h) = self.history.get_mut(&repo) else {
					return;
				};
				h.loading = false;
				match result {
					Ok(commits) => {
						if commits.len() < PAGE {
							h.exhausted = true;
						}
						if skip == 0 {
							h.commits = commits;
						} else {
							h.commits.extend(commits);
						}
					}
					Err(e) => h.error = Some(e),
				}
			}
			HistoryMsg::Files { repo, hash, result } => {
				if let Some(h) = self.history.get_mut(&repo) {
					match result {
						Ok(files) => {
							h.files.insert(hash, files);
						}
						Err(e) => h.error = Some(e),
					}
				}
			}
		}
	}
}

fn spawn_page(repo: PathBuf, skip: usize, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let result = read_log(&repo, skip).map_err(|e| e.to_string());
		let _ = tx.send(Msg::History(HistoryMsg::Page { repo, skip, result }));
		ctx.request_repaint();
	});
}

fn spawn_files(repo: PathBuf, hash: String, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let result = read_commit_files(&repo, &hash).map_err(|e| e.to_string());
		let _ = tx.send(Msg::History(HistoryMsg::Files { repo, hash, result }));
		ctx.request_repaint();
	});
}
