//! Merging scan results into the rows by repository path, so a refresh never resets the selection
//! or scroll, and the lookups the screens share.

use std::path::Path;

use repose_core::model::{RepoSummary, StatusSummary};

use super::AppState;

/// What a status merge did.
pub(super) struct Merged {
	/// The read was accepted and something on the row differs from before: branch, hash,
	/// counts, file list or error.
	pub changed: bool,
	pub head_changed: bool,
}

impl AppState {
	/// Replaces the rows with a fresh discovery. Returns false when a command-line selection was
	/// asked for and not found. An open diff or file follows the same rule as `select`: it closes
	/// when the selection moves to another repository or its own repository is gone, so a file
	/// from a vanished repository is never drawn under another one's header (Roger on e176de4).
	pub(super) fn merge_discovered(&mut self, fresh: Vec<RepoSummary>) -> bool {
		let before = self.selected.clone();
		let landed = self.replace_rows(fresh);
		let orphaned = |repo: &Path| {
			self.selected != before && self.selected.as_deref() != Some(repo)
				|| find(&self.repos, repo).is_none()
		};
		let close_diff = self.diff.as_ref().is_some_and(|d| orphaned(&d.open.repo));
		let close_file = self
			.file_view
			.as_ref()
			.is_some_and(|v| orphaned(&v.open.repo));
		if close_diff {
			self.close_diff();
		}
		if close_file {
			self.close_file();
		}
		landed
	}

	fn replace_rows(&mut self, mut fresh: Vec<RepoSummary>) -> bool {
		// Keep statuses already known for paths that are still present.
		for row in &mut fresh {
			if let Some(old) = find(&self.repos, &row.path) {
				row.status = old.status.clone();
				row.changes = old.changes.clone();
				row.error = old.error.clone();
				row.checked_at = old.checked_at;
			}
			for wt in &mut row.worktrees {
				if let Some(old) = find(&self.repos, &wt.path) {
					wt.status = old.status.clone();
					wt.changes = old.changes.clone();
					wt.error = old.error.clone();
				}
			}
		}
		self.repos = fresh;
		let mut landed = true;
		if let Some(wanted) = self.requested_selection.take() {
			let key = |p: &Path| p.to_string_lossy().to_lowercase().replace('\\', "/");
			let hit = self
				.repos
				.iter()
				.flat_map(|r| std::iter::once(r).chain(r.worktrees.iter()))
				.find(|r| key(&r.path) == key(&wanted))
				.map(|r| r.path.clone());
			landed = hit.is_some();
			if landed {
				self.selected = hit;
			}
		}
		if self
			.selected
			.as_ref()
			.is_none_or(|p| find(&self.repos, p).is_none())
		{
			self.selected = self.repos.first().map(|r| r.path.clone());
		}
		landed
	}

	/// Merges a status read into its row and says whether it applied and whether HEAD, the
	/// upstream or the ahead count changed, which is when a loaded history is stale.
	pub(super) fn merge_status(&mut self, fresh: RepoSummary) -> Merged {
		let none = Merged {
			changed: false,
			head_changed: false,
		};
		let Some(row) = find_mut(&mut self.repos, &fresh.path) else {
			return none;
		};
		// A full Refresh can overlap a post-write re-read; the one that started reading later
		// wins, so an older scan result never overwrites a newer status.
		if row.checked_at > fresh.checked_at {
			return none;
		}
		let tracking =
			|s: &Option<StatusSummary>| s.as_ref().map(|s| (s.upstream.clone(), s.ahead));
		let head_changed = fresh.short_hash.is_some() && fresh.short_hash != row.short_hash
			|| tracking(&fresh.status) != tracking(&row.status);
		let changed = fresh.branch != row.branch
			|| (fresh.short_hash.is_some() && fresh.short_hash != row.short_hash)
			|| fresh.status != row.status
			|| fresh.changes != row.changes
			|| fresh.error != row.error;
		row.branch = fresh.branch;
		row.short_hash = fresh.short_hash.or(row.short_hash.take());
		row.status = fresh.status;
		row.changes = fresh.changes;
		row.error = fresh.error;
		row.checked_at = fresh.checked_at;
		Merged {
			changed,
			head_changed,
		}
	}
}

pub fn find<'a>(repos: &'a [RepoSummary], path: &Path) -> Option<&'a RepoSummary> {
	for r in repos {
		if r.path == path {
			return Some(r);
		}
		if let Some(w) = r.worktrees.iter().find(|w| w.path == path) {
			return Some(w);
		}
	}
	None
}

fn find_mut<'a>(repos: &'a mut [RepoSummary], path: &Path) -> Option<&'a mut RepoSummary> {
	for r in repos.iter_mut() {
		if r.path == path {
			return Some(r);
		}
		if let Some(w) = r.worktrees.iter_mut().find(|w| w.path == path) {
			return Some(w);
		}
	}
	None
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use repose_core::config::Config;
	use repose_core::model::{DiffSource, RepoKind, RepoSummary};

	use crate::state::{
		AppState, DiffContent, DiffState, FileContent, FileRev, FileViewState, OpenDiff, OpenFile,
	};

	fn repo(name: &str) -> RepoSummary {
		RepoSummary::new(PathBuf::from(format!("D:/t/{name}")), RepoKind::Repository)
	}

	/// Roger on e176de4: with A selected and its file open, a discovery that no longer lists A
	/// selects B; the file (like a diff) must close rather than draw under B's header.
	#[test]
	fn a_vanished_repository_closes_its_file_and_diff() {
		let mut state = AppState::new(Config::default());
		state.merge_discovered(vec![repo("a"), repo("b")]);
		state.selected = Some(repo("a").path);
		state.file_view = Some(FileViewState {
			open: OpenFile {
				repo: repo("a").path,
				rev: FileRev::Head,
				path: "x.txt".into(),
				size: Some(1),
			},
			content: FileContent::Loading,
			generation: 1,
			current_line: None,
			notice: None,
		});
		state.diff = Some(DiffState {
			open: OpenDiff {
				repo: repo("a").path,
				source: DiffSource::Working,
				paths: vec!["x.txt".into()],
				display: "x.txt".into(),
			},
			content: DiffContent::Loading,
			inline: false,
			generation: 1,
			rows: Vec::new(),
			rows_inline: false,
			colours: Vec::new(),
		});
		state.merge_discovered(vec![repo("b")]);
		assert_eq!(state.selected, Some(repo("b").path));
		assert!(state.file_view.is_none(), "the file from A is closed");
		assert!(state.diff.is_none(), "the diff from A is closed");

		// The same discovery with A still present keeps both open.
		state.selected = Some(repo("b").path);
		state.file_view = Some(FileViewState {
			open: OpenFile {
				repo: repo("b").path,
				rev: FileRev::Head,
				path: "y.txt".into(),
				size: Some(1),
			},
			content: FileContent::Loading,
			generation: 2,
			current_line: None,
			notice: None,
		});
		state.merge_discovered(vec![repo("a"), repo("b")]);
		assert_eq!(state.selected, Some(repo("b").path));
		assert!(
			state.file_view.is_some(),
			"B is still there, so its file stays"
		);
	}
}
