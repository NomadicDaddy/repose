//! The repository list's parent and worktree rows (screens/repo_list.rs row_events and
//! apply_events), driven through the real frame: the chevron opens and closes a parent's worktree
//! rows without moving the selection, a filter opens a parent to the worktrees whose names match
//! and the chevron then closes and reopens it fully, and a click on a worktree row selects it.

use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;

use super::AppState;
use super::ui_harness::{click, frame, repo, text_at, texts};

/// One parent, `p`, with two worktrees, and a second repository below it.
fn with_worktrees() -> AppState {
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	let mut parent = repo("p", t0);
	parent.worktrees.push(repo("alpha-wt", t0));
	parent.worktrees.push(repo("beta-wt", t0));
	state.merge_discovered(vec![parent, repo("q", t0)]);
	state
}

/// Clicks the painted text `label` and draws the frame that applies the click.
fn click_text(ctx: &egui::Context, state: &mut AppState, label: &str) {
	let shapes = frame(ctx, state, Vec::new());
	let Some(at) = text_at(&shapes, label) else {
		panic!("{label:?} not painted: {:?}", texts(&shapes));
	};
	frame(ctx, state, click(at));
}

fn shown(ctx: &egui::Context, state: &mut AppState) -> Vec<String> {
	texts(&frame(ctx, state, Vec::new()))
}

#[test]
fn the_chevron_opens_and_closes_the_worktree_rows_and_keeps_the_selection() {
	let ctx = egui::Context::default();
	let mut state = with_worktrees();
	let before = shown(&ctx, &mut state);
	assert!(before.iter().any(|t| t == "▸ 2 worktrees"), "{before:?}");
	assert!(
		!before.iter().any(|t| t == "alpha-wt"),
		"starts collapsed: {before:?}"
	);
	let selected = state.selected.clone();
	assert!(selected.is_some(), "the first row is selected");

	click_text(&ctx, &mut state, "▸ 2 worktrees");
	let open = shown(&ctx, &mut state);
	assert!(open.iter().any(|t| t == "▾ 2 worktrees"), "{open:?}");
	assert!(open.iter().any(|t| t == "alpha-wt"), "{open:?}");
	assert!(open.iter().any(|t| t == "beta-wt"), "{open:?}");
	assert_eq!(state.selected, selected, "the chevron is not a row click");

	click_text(&ctx, &mut state, "▾ 2 worktrees");
	let closed = shown(&ctx, &mut state);
	assert!(closed.iter().any(|t| t == "▸ 2 worktrees"), "{closed:?}");
	assert!(
		!closed.iter().any(|t| t == "alpha-wt" || t == "beta-wt"),
		"{closed:?}"
	);
	assert_eq!(state.selected, selected);
}

#[test]
fn a_filter_opens_the_matching_worktree_and_the_chevron_then_closes_and_reopens_fully() {
	let ctx = egui::Context::default();
	let mut state = with_worktrees();
	state.filter = "alpha".into();
	state.filter_changed();
	let filtered = shown(&ctx, &mut state);
	assert!(filtered.iter().any(|t| t == "alpha-wt"), "{filtered:?}");
	assert!(!filtered.iter().any(|t| t == "beta-wt"), "{filtered:?}");
	assert!(!filtered.iter().any(|t| t == "q"), "{filtered:?}");
	assert!(
		filtered.iter().any(|t| t == "▾ 2 worktrees"),
		"reads open: {filtered:?}"
	);

	click_text(&ctx, &mut state, "▾ 2 worktrees");
	let closed = shown(&ctx, &mut state);
	assert!(!closed.iter().any(|t| t == "alpha-wt"), "{closed:?}");
	assert!(closed.iter().any(|t| t == "▸ 2 worktrees"), "{closed:?}");

	click_text(&ctx, &mut state, "▸ 2 worktrees");
	let reopened = shown(&ctx, &mut state);
	assert!(reopened.iter().any(|t| t == "alpha-wt"), "{reopened:?}");
	assert!(
		reopened.iter().any(|t| t == "beta-wt"),
		"fully, past the filter: {reopened:?}"
	);
}

#[test]
fn a_click_on_a_worktree_row_selects_that_worktree() {
	let ctx = egui::Context::default();
	let mut state = with_worktrees();
	click_text(&ctx, &mut state, "▸ 2 worktrees");
	let wt = state.repos[0].worktrees[1].path.clone();
	assert_ne!(state.selected.as_ref(), Some(&wt));
	click_text(&ctx, &mut state, "beta-wt");
	assert_eq!(state.selected, Some(wt));
}
