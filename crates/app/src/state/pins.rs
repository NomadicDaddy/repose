//! Pinned repositories in the list (repository-pinning clauses 2 to 5): which path a row's Pin
//! acts on, whether a top-level row is pinned (its own path, or a saved pin of one of its
//! worktrees from when that worktree stood alone), pinned rows first, and the toggle that saves
//! the new list to the selected config.toml before it changes anything on screen.

use std::path::{Path, PathBuf};

use repose_core::model::RepoSummary;
use repose_core::pins;

use super::AppState;

impl AppState {
	/// The top-level row a Pin on `path` acts on: the row itself, or the parent a nested worktree
	/// is drawn under. A worktree promoted to a top-level row (its parent outside the roots) is
	/// its own row. None for a path no row shows.
	pub fn pin_row(&self, path: &Path) -> Option<&RepoSummary> {
		self.repos
			.iter()
			.find(|r| r.path == path || r.worktrees.iter().any(|w| w.path == path))
	}

	/// A top-level row is pinned by its own saved path, or by a saved pin of one of its worktrees:
	/// a worktree pinned while it stood alone lifts its parent once the parent is discovered.
	pub fn row_pinned(&self, row: &RepoSummary) -> bool {
		let saved = &self.config.pinned_repositories;
		pins::is_pinned(saved, &row.path)
			|| row
				.worktrees
				.iter()
				.any(|w| pins::is_pinned(saved, &w.path))
	}

	/// The menu item for the row at `path`: Pin or Unpin on a top-level row, and Pin parent
	/// repository or Unpin parent repository on a worktree drawn under its parent.
	pub fn pin_label(&self, path: &Path) -> Option<&'static str> {
		let row = self.pin_row(path)?;
		let nested = row.path != path;
		Some(match (self.row_pinned(row), nested) {
			(false, false) => "Pin",
			(true, false) => "Unpin",
			(false, true) => "Pin parent repository",
			(true, true) => "Unpin parent repository",
		})
	}

	/// Pins or unpins the row `path` belongs to. Unpin also clears a pin the row inherited from one
	/// of its worktrees. The new list is saved first; a failed save returns its error, for the
	/// menu to show, and leaves the pins and the order as they were. A save needs the config path
	/// main.rs passes; without one (the UI tests' default) the change lasts for the session only.
	pub fn toggle_pin(&mut self, path: &Path) -> Result<(), String> {
		let Some(row) = self.pin_row(path) else {
			return Ok(());
		};
		let saved = &self.config.pinned_repositories;
		let next: Vec<PathBuf> = if self.row_pinned(row) {
			let mut left = pins::unpinned(saved, &row.path);
			for w in &row.worktrees {
				left = pins::unpinned(&left, &w.path);
			}
			left
		} else {
			pins::pinned(saved, &row.path)
		};
		if let Some(file) = &self.config_path {
			pins::save(file, &next).map_err(|e| format!("The pin was not saved: {e}"))?;
		}
		self.config.pinned_repositories = next;
		self.order_pinned();
		self.reveal_selected = true;
		Ok(())
	}

	/// Pinned top-level rows first, then the rest, each part by name without regard to case, as
	/// discovery orders them, so a row unpinned goes back to its place; worktrees stay under their
	/// rows.
	pub(super) fn order_pinned(&mut self) {
		let mut repos = std::mem::take(&mut self.repos);
		repos.sort_by_cached_key(|r| (!self.row_pinned(r), r.name.to_lowercase()));
		self.repos = repos;
	}
}
