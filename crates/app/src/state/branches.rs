//! The Branches tab's state: one list per repository, read on a thread when the tab first shows
//! the repository and re-read when a status lands with a changed HEAD or changed counts; never
//! on a timer of its own (branches-list clause 4).

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::branches::read_branches;
use repose_core::model::Branches;

use super::{AppState, Msg};

#[derive(Default)]
pub struct BranchesState {
	/// Bumped by every re-read; a result carrying an older value is dropped.
	pub generation: u64,
	/// The last list read; kept on screen while a re-read runs.
	pub branches: Option<Branches>,
	pub loading: bool,
	pub error: Option<String>,
}

pub struct BranchesMsg {
	pub repo: PathBuf,
	pub generation: u64,
	pub result: Result<Branches, String>,
}

impl AppState {
	pub fn branches_of(&self, repo: &Path) -> Option<&BranchesState> {
		self.branches.get(repo)
	}

	/// Reads the list when the tab first shows a repository.
	pub fn ensure_branches(&mut self, repo: &Path, ctx: &egui::Context) {
		if self.branches.contains_key(repo) {
			return;
		}
		self.branches
			.insert(repo.to_path_buf(), BranchesState::default());
		self.load_branches(repo, ctx);
	}

	/// Re-reads a loaded list when the repository's status changed: a commit, a checkout or a
	/// fetch from a terminal moves HEAD or the saved counts, and Refresh re-reads every status.
	pub(super) fn refresh_branches(&mut self, repo: &Path, ctx: &egui::Context) {
		let Some(b) = self.branches.get_mut(repo) else {
			return;
		};
		b.generation += 1;
		b.loading = false;
		self.load_branches(repo, ctx);
	}

	fn load_branches(&mut self, repo: &Path, ctx: &egui::Context) {
		let Some(b) = self.branches.get_mut(repo) else {
			return;
		};
		if b.loading {
			return;
		}
		b.loading = true;
		spawn_read(
			repo.to_path_buf(),
			b.generation,
			self.tx.clone(),
			ctx.clone(),
		);
	}

	pub(super) fn merge_branches(&mut self, msg: BranchesMsg) {
		let Some(b) = self.branches.get_mut(&msg.repo) else {
			return;
		};
		if msg.generation != b.generation {
			return;
		}
		b.loading = false;
		match msg.result {
			Ok(list) => {
				b.error = None;
				b.branches = Some(list);
			}
			Err(e) => b.error = Some(e),
		}
	}
}

fn spawn_read(repo: PathBuf, generation: u64, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let result = read_branches(&repo).map_err(|e| e.to_string());
		let _ = tx.send(Msg::Branches(BranchesMsg {
			repo,
			generation,
			result,
		}));
		ctx.request_repaint();
	});
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use repose_core::config::Config;
	use repose_core::model::{Branch, Branches, HeadState};

	use super::BranchesMsg;
	use crate::state::AppState;

	fn list(name: &str) -> Branches {
		Branches {
			head: HeadState::OnBranch(name.into()),
			list: vec![Branch {
				name: name.into(),
				short_hash: "0123abc".into(),
				upstream: None,
				ahead: None,
				behind: None,
				current: true,
			}],
		}
	}

	/// A result from before a re-read never replaces the newer list, and an error keeps the
	/// last list on screen beside its message (branches-list clause 4).
	#[test]
	fn a_stale_result_is_dropped_and_an_error_keeps_the_last_list() {
		let mut state = AppState::new(Config::default());
		let repo = PathBuf::from("D:/t/one");
		state
			.branches
			.insert(repo.clone(), super::BranchesState::default());
		state.merge_branches(BranchesMsg {
			repo: repo.clone(),
			generation: 0,
			result: Ok(list("main")),
		});
		assert_eq!(
			state.branches_of(&repo).and_then(|b| b.branches.as_ref()),
			Some(&list("main"))
		);
		// A re-read was started (generation 1); the old read's answer arrives late.
		if let Some(b) = state.branches.get_mut(&repo) {
			b.generation = 1;
		}
		state.merge_branches(BranchesMsg {
			repo: repo.clone(),
			generation: 0,
			result: Ok(list("stale")),
		});
		assert_eq!(
			state.branches_of(&repo).and_then(|b| b.branches.as_ref()),
			Some(&list("main")),
			"the stale page is dropped"
		);
		state.merge_branches(BranchesMsg {
			repo: repo.clone(),
			generation: 1,
			result: Err("git failed".into()),
		});
		let b = state.branches_of(&repo).unwrap_or_else(|| panic!("state"));
		assert_eq!(b.error.as_deref(), Some("git failed"));
		assert_eq!(b.branches, Some(list("main")), "the last list stays");
	}
}
