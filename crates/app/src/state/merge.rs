//! Merging scan results into the rows by repository path, so a refresh never resets the selection
//! or scroll, and the lookups the screens share.

use std::path::Path;

use repose_core::model::{RepoSummary, StatusSummary};

use super::AppState;

/// What a status merge did.
pub(super) struct Merged {
	/// The read was accepted (not older than the row's).
	pub applied: bool,
	/// Something on the row differs from before: branch, hash, counts, file list or error. A
	/// second edit to an already-modified file changes none of these.
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
					wt.checked_at = old.checked_at;
				}
			}
		}
		self.repos = fresh;
		self.order_pinned();
		let mut landed = true;
		if let Some(wanted) = self.requested_selection.take() {
			let key = repose_core::model::path_key;
			let hit = self.repos.iter().find_map(|r| {
				if key(&r.path) == key(&wanted) {
					return Some((r.path.clone(), None));
				}
				r.worktrees
					.iter()
					.find(|w| key(&w.path) == key(&wanted))
					.map(|w| (w.path.clone(), Some(r.path.clone())))
			});
			landed = hit.is_some();
			if let Some((path, parent)) = hit {
				// A linked worktree's row is drawn only under an expanded parent, so a start
				// from inside one opens the parent too (Morticia Low on dd01d61).
				if let Some(parent) = parent {
					self.expanded.insert(parent, true);
				}
				self.selected = Some(path);
				self.reveal_selected = true;
			}
		}
		if self
			.selected
			.as_ref()
			.is_none_or(|p| find(&self.repos, p).is_none())
		{
			self.selected = self.repos.first().map(|r| r.path.clone());
			self.reveal_selected = true;
		}
		landed
	}

	/// Merges a status read into its row and says whether it applied and whether HEAD, the
	/// upstream or the ahead count changed, which is when a loaded history is stale.
	pub(super) fn merge_status(&mut self, fresh: RepoSummary) -> Merged {
		let none = Merged {
			applied: false,
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
			applied: true,
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
	use std::time::{Duration, SystemTime};

	use repose_core::config::Config;
	use repose_core::model::{DiffSource, RepoKind, RepoSummary, StatusSummary};

	use crate::state::{
		AppState, DiffContent, DiffState, FileContent, FileRev, FileViewState, OpenDiff, OpenFile,
	};

	fn repo(name: &str) -> RepoSummary {
		RepoSummary::new(PathBuf::from(format!("D:/t/{name}")), RepoKind::Repository)
	}

	#[test]
	fn rescan_keeps_nested_worktree_freshness() {
		let mut state = AppState::new(Config::default());
		let mut main = repo("a");
		let path = PathBuf::from("D:/t/a-wt");
		let t1 = SystemTime::UNIX_EPOCH + Duration::from_secs(1);
		let t2 = t1 + Duration::from_secs(1);
		let mut current = RepoSummary::new(path.clone(), RepoKind::Worktree);
		current.status = Some(StatusSummary {
			staged: 1,
			..StatusSummary::default()
		});
		current.checked_at = Some(t2);
		main.worktrees.push(current);
		state.merge_discovered(vec![main]);

		let mut discovered = repo("a");
		discovered
			.worktrees
			.push(RepoSummary::new(path.clone(), RepoKind::Worktree));
		state.merge_discovered(vec![discovered]);
		let mut stale = RepoSummary::new(path.clone(), RepoKind::Worktree);
		stale.status = Some(StatusSummary {
			staged: 2,
			..StatusSummary::default()
		});
		stale.checked_at = Some(t1);
		assert!(!state.merge_status(stale).applied);
		let Some(retained) = super::find(&state.repos, &path) else {
			panic!("worktree remains after rescan");
		};
		assert_eq!(retained.status.as_ref().map(|s| s.staged), Some(1));
		assert_eq!(retained.checked_at, Some(t2));
	}

	/// Morticia Low on dd01d61: a start from inside a linked worktree selects a row that is drawn
	/// only under an expanded parent, so landing there opens the parent and asks to be shown.
	#[test]
	fn a_requested_worktree_expands_its_parent_when_it_lands() {
		let mut state = AppState::new(Config::default());
		let mut main = repo("a");
		main.worktrees.push(RepoSummary::new(
			PathBuf::from("D:/t/a-wt"),
			RepoKind::Worktree,
		));
		state.requested_selection = Some(PathBuf::from("d:\\t\\A-WT"));
		assert!(state.merge_discovered(vec![main, repo("b")]));
		assert_eq!(state.selected, Some(PathBuf::from("D:/t/a-wt")));
		assert_eq!(
			state.expanded.get(&PathBuf::from("D:/t/a")),
			Some(&true),
			"the parent is open"
		);
		assert!(state.reveal_selected);
		assert!(
			state.visible_paths().contains(&PathBuf::from("D:/t/a-wt")),
			"the worktree row is drawn"
		);
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
			raw_hash: None,
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
