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
			// A repository that matches the filter itself carries every worktree when opened: the
			// chevron counts them all, so opening it must show them all, not only the ones whose
			// own names happen to match (Phillip #31154: "22 worktrees" opened to nothing while
			// the list was filtered to that repository). A match on a worktree's name alone
			// opens the parent to the matching ones.
			let opened = self.expanded.contains(&r.path);
			let auto = !needle.is_empty() && !matching_wts.is_empty();
			if opened || auto {
				let shown: Vec<&RepoSummary> = if opened && matches_self {
					r.worktrees.iter().collect()
				} else {
					matching_wts
				};
				for w in shown {
					out.push(w.path.clone());
				}
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
	fn an_opened_repository_that_matches_the_filter_shows_every_worktree() {
		let mut state = fixture();
		state.filter = "summon".into();
		assert_eq!(state.visible_paths().len(), 1, "closed: the parent alone");
		state.expanded.insert(PathBuf::from("D:/t/summon"));
		let paths = state.visible_paths();
		assert_eq!(
			paths.len(),
			4,
			"opened: the parent and all three worktrees: {paths:?}"
		);
	}

	/// A filter that matches a worktree's name alone opens the parent to the matching ones.
	#[test]
	fn a_worktree_name_match_opens_its_parent_to_the_matches() {
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
		// Opened by hand under that filter it still shows only the matches: the parent does not
		// match, so there is nothing to carry.
		state.expanded.insert(PathBuf::from("D:/t/summon"));
		assert_eq!(state.visible_paths().len(), 2);
	}
}
