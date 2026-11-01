//! Which rows are visible after the filter, and keyboard movement over them.

use std::path::PathBuf;

use repose_core::model::RepoSummary;

use super::AppState;

impl AppState {
	/// Rows in display order after the filter: top-level repositories plus the worktrees of the
	/// expanded ones. Used for both drawing and keyboard movement.
	pub fn visible_paths(&self) -> Vec<PathBuf> {
		let needle = self.filter.trim().to_lowercase();
		let mut out = Vec::new();
		for r in &self.repos {
			let matches_self = needle.is_empty() || r.name.to_lowercase().contains(&needle);
			let matching_wts: Vec<&RepoSummary> = r
				.worktrees
				.iter()
				.filter(|w| needle.is_empty() || w.name.to_lowercase().contains(&needle))
				.collect();
			if !matches_self && matching_wts.is_empty() {
				continue;
			}
			out.push(r.path.clone());
			// The chevron counts every worktree, so a row the user opened shows them all, and a
			// row the user closed shows none, whatever the filter (Phillip #31154: "22
			// worktrees" opened to nothing while the list was filtered to that repository;
			// Morticia L-6). A row the user has not touched is left to the filter: open to the
			// worktrees whose own names match while a filter is typed, closed otherwise.
			let shown: Vec<&RepoSummary> = match self.expanded.get(&r.path) {
				Some(true) => r.worktrees.iter().collect(),
				Some(false) => Vec::new(),
				None if !needle.is_empty() => matching_wts,
				None => Vec::new(),
			};
			for w in shown {
				out.push(w.path.clone());
			}
		}
		out
	}

	pub fn move_selection(&mut self, delta: i32) {
		let paths = self.visible_paths();
		if paths.is_empty() {
			return;
		}
		let current = self
			.selected
			.as_ref()
			.and_then(|s| paths.iter().position(|p| p == s));
		let next = match current {
			Some(i) => (i as i32 + delta).clamp(0, paths.len() as i32 - 1) as usize,
			None => 0,
		};
		if let Some(p) = paths.get(next).cloned() {
			self.select(p);
			// A keyboard move can land on a row outside the view; the list brings it in once.
			self.reveal_selected = true;
		}
	}
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use repose_core::config::Config;
	use repose_core::model::{RepoKind, RepoSummary};

	use crate::state::AppState;

	fn fixture() -> AppState {
		let mut state = AppState::new(Config::default());
		let mut summon = RepoSummary::new(PathBuf::from("D:/t/summon"), RepoKind::Repository);
		for name in ["identity-banner", "stop-confirm", "radio-proof"] {
			summon.worktrees.push(RepoSummary::new(
				PathBuf::from(format!("D:/w/summon/{name}")),
				RepoKind::Worktree,
			));
		}
		state.repos = vec![
			summon,
			RepoSummary::new(PathBuf::from("D:/t/other"), RepoKind::Repository),
		];
		state
	}

	/// Phillip #31154: with the list filtered to "summon", opening the row that says "3
	/// worktrees" showed nothing, because only worktrees whose own names matched were listed.
	#[test]
	fn an_opened_repository_shows_every_worktree_whatever_the_filter() {
		let mut state = fixture();
		state.filter = "summon".into();
		assert_eq!(
			state.visible_paths().len(),
			1,
			"untouched: the parent alone"
		);
		state.expanded.insert(PathBuf::from("D:/t/summon"), true);
		let paths = state.visible_paths();
		assert_eq!(
			paths.len(),
			4,
			"opened: the parent and all three worktrees: {paths:?}"
		);
		state.expanded.insert(PathBuf::from("D:/t/summon"), false);
		assert_eq!(
			state.visible_paths().len(),
			1,
			"closed: the parent alone again"
		);
	}

	/// A filter that matches a worktree's name alone opens an untouched parent to the matching
	/// ones; the user's open or close then overrides it (Morticia L-6 on 4dc97d1).
	#[test]
	fn a_worktree_name_match_opens_an_untouched_parent_to_the_matches() {
		let mut state = fixture();
		state.filter = "stop".into();
		let paths = state.visible_paths();
		assert_eq!(
			paths,
			vec![
				PathBuf::from("D:/t/summon"),
				PathBuf::from("D:/w/summon/stop-confirm")
			]
		);
		state.expanded.insert(PathBuf::from("D:/t/summon"), false);
		assert_eq!(
			state.visible_paths().len(),
			1,
			"closed by hand: no worktree rows"
		);
		state.expanded.insert(PathBuf::from("D:/t/summon"), true);
		assert_eq!(state.visible_paths().len(), 4, "opened by hand: all three");
	}
}
