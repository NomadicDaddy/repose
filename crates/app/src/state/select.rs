//! The selected repository, which rows are visible after the filter, and keyboard movement over
//! them.

use std::path::PathBuf;

use repose_core::model::RepoSummary;

use super::{AppState, find};

impl AppState {
	pub fn selected_repo(&self) -> Option<&RepoSummary> {
		self.selected.as_ref().and_then(|p| find(&self.repos, p))
	}

	/// Selects a repository. An open diff or file belongs to the repository it was opened on, so
	/// moving to another closes it rather than drawing it under the new header. Choosing a
	/// repository leaves the Activity screen (activity-screen clause 1).
	pub fn select(&mut self, path: PathBuf) {
		self.activity.shown = false;
		if self.selected.as_ref() != Some(&path) {
			self.selected_file = None;
			if self.diff.as_ref().is_some_and(|d| d.open.repo != path) {
				self.close_diff();
			}
			if self.file_view.as_ref().is_some_and(|v| v.open.repo != path) {
				self.close_file();
			}
		}
		self.selected = Some(path);
	}

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

	/// The filter text changed: a row the user closed by hand stays closed only while the filter
	/// it was closed under stands, so a later filter that matches one of its worktrees shows
	/// that worktree again instead of a bare parent with nothing to say why (Morticia Low on
	/// d9e6939). Rows the user opened stay open.
	pub fn filter_changed(&mut self) {
		self.expanded.retain(|_, open| *open);
	}

	/// A pause this long ends the typed prefix; the next letter starts a new one.
	pub const TYPE_AHEAD_RESET: std::time::Duration = std::time::Duration::from_secs(1);

	/// Letters typed with the list focused (Phillip #31623): the prefix grows while the letters
	/// keep coming and starts over after a second's pause; the first visible row whose name
	/// starts with it, case-insensitive, is selected and scrolled into view. No match leaves the
	/// selection where it is, and the prefix still grows, so a mistyped letter followed by the
	/// right ones can still land once the pause resets it.
	pub fn type_ahead(&mut self, text: &str, now: std::time::Instant) {
		let stale = self
			.type_ahead_at
			.is_none_or(|at| now.duration_since(at) > Self::TYPE_AHEAD_RESET);
		if stale {
			self.type_ahead.clear();
		}
		self.type_ahead.push_str(text);
		self.type_ahead_at = Some(now);
		let needle = self.type_ahead.to_lowercase();
		let hit = self.visible_paths().into_iter().find(|p| {
			super::find(&self.repos, p).is_some_and(|r| r.name.to_lowercase().starts_with(&needle))
		});
		if let Some(p) = hit {
			self.select(p);
			self.reveal_selected = true;
		}
	}

	/// Escape with the list focused forgets the typed prefix at once.
	pub fn type_ahead_clear(&mut self) {
		self.type_ahead.clear();
		self.type_ahead_at = None;
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

	/// Morticia Low on d9e6939: a hand-close lasts while that filter stands; a new filter that
	/// matches a worktree shows it again. A hand-open outlives the filter change.
	#[test]
	fn a_closed_row_reopens_to_matches_when_the_filter_changes() {
		let mut state = fixture();
		state.filter = "summon".into();
		state.expanded.insert(PathBuf::from("D:/t/summon"), false);
		state.filter = "stop".into();
		state.filter_changed();
		assert_eq!(
			state.visible_paths(),
			vec![
				PathBuf::from("D:/t/summon"),
				PathBuf::from("D:/w/summon/stop-confirm")
			],
			"the close was dropped with the filter it was made under"
		);
		state.expanded.insert(PathBuf::from("D:/t/summon"), true);
		state.filter = "sum".into();
		state.filter_changed();
		assert_eq!(state.visible_paths().len(), 4, "a hand-open stays open");
	}

	/// Phillip #31623: typing with the list focused jumps to the first visible row whose name
	/// starts with the letters so far; a pause over a second starts a new prefix; a worktree row
	/// counts once it is showing; no match leaves the selection alone.
	#[test]
	fn typing_selects_the_first_row_whose_name_starts_with_the_letters() {
		use std::time::{Duration, Instant};
		let mut state = fixture();
		let t0 = Instant::now();
		state.type_ahead("o", t0);
		assert_eq!(
			state.selected.as_deref(),
			Some(PathBuf::from("D:/t/other").as_path())
		);
		assert!(state.reveal_selected, "the jump scrolls the row into view");
		state.reveal_selected = false;
		state.type_ahead("t", t0 + Duration::from_millis(300));
		assert_eq!(
			state.type_ahead, "ot",
			"the prefix grows while the keys keep coming"
		);
		assert_eq!(
			state.selected.as_deref(),
			Some(PathBuf::from("D:/t/other").as_path())
		);
		state.type_ahead("s", t0 + Duration::from_millis(1500));
		assert_eq!(state.type_ahead, "s", "a pause over a second starts over");
		assert_eq!(
			state.selected.as_deref(),
			Some(PathBuf::from("D:/t/summon").as_path())
		);
		state.reveal_selected = false;
		state.type_ahead("zz", t0 + Duration::from_millis(3000));
		assert_eq!(
			state.selected.as_deref(),
			Some(PathBuf::from("D:/t/summon").as_path()),
			"no match leaves the selection where it is"
		);
		assert!(!state.reveal_selected, "and asks for no scroll");
		state.type_ahead_clear();
		assert!(state.type_ahead.is_empty() && state.type_ahead_at.is_none());
		// A worktree row joins the candidates once its parent is open.
		state.type_ahead("st", t0 + Duration::from_millis(5000));
		assert_eq!(
			state.selected.as_deref(),
			Some(PathBuf::from("D:/t/summon").as_path())
		);
		state.expanded.insert(PathBuf::from("D:/t/summon"), true);
		state.type_ahead("st", t0 + Duration::from_millis(7000));
		assert_eq!(
			state.selected.as_deref(),
			Some(PathBuf::from("D:/w/summon/stop-confirm").as_path())
		);
	}
}
