//! Merging scan results into the rows by repository path, so a refresh never resets the selection
//! or scroll, and the lookups the screens share.

use std::path::Path;

use repose_core::model::{RepoSummary, StatusSummary};

use super::AppState;

/// What a status merge did.
pub(super) struct Merged {
	pub applied: bool,
	pub head_changed: bool,
}

impl AppState {
	/// Replaces the rows with a fresh discovery. Returns false when a command-line selection was
	/// asked for and not found. Any open diff follows the same rule as `select`: it closes when
	/// the selection moves to another repository or its own repository is gone.
	pub(super) fn merge_discovered(&mut self, fresh: Vec<RepoSummary>) -> bool {
		let before = self.selected.clone();
		let landed = self.replace_rows(fresh);
		if let Some(d) = &self.diff
			&& (self.selected != before && self.selected.as_ref() != Some(&d.open.repo)
				|| find(&self.repos, &d.open.repo).is_none())
		{
			self.close_diff();
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
			applied: false,
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
		row.branch = fresh.branch;
		row.short_hash = fresh.short_hash.or(row.short_hash.take());
		row.status = fresh.status;
		row.changes = fresh.changes;
		row.error = fresh.error;
		row.checked_at = fresh.checked_at;
		Merged {
			applied: true,
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
