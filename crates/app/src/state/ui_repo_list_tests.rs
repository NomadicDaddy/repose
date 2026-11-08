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

/// The painted chevron: a filled three-point shape. Returns whether it points down (open), its
/// height, and its vertical centre.
fn chevron(shapes: &[egui::epaint::ClippedShape]) -> Option<(bool, f32, f32)> {
	shapes.iter().find_map(|s| match &s.shape {
		egui::epaint::Shape::Path(p) if p.points.len() == 3 && p.closed => {
			let ys: Vec<f32> = p.points.iter().map(|q| q.y).collect();
			let (top, bottom) = (
				ys.iter().cloned().fold(f32::MAX, f32::min),
				ys.iter().cloned().fold(f32::MIN, f32::max),
			);
			// Pointing down: a flat top edge, two points sharing the top y. Pointing right has a flat
			// left edge instead, and one point at each of the top and bottom.
			let flat_top = ys.iter().filter(|y| (**y - top).abs() < 0.01).count() == 2;
			Some((flat_top, bottom - top, (top + bottom) / 2.0))
		}
		_ => None,
	})
}

fn open_state(ctx: &egui::Context, state: &mut AppState) -> (Vec<String>, Option<bool>) {
	let shapes = frame(ctx, state, Vec::new());
	(texts(&shapes), chevron(&shapes).map(|c| c.0))
}

/// Design-polish clause 5: the chevron is a painted triangle sized and centred on its
/// "<n> worktrees" text, not a font glyph that came out small and high.
#[test]
fn the_chevron_is_sized_and_aligned_with_its_text() {
	let ctx = egui::Context::default();
	crate::theme::apply(&ctx, repose_core::config::Theme::Dark);
	let mut state = with_worktrees();
	let shapes = frame(&ctx, &mut state, Vec::new());
	let all = texts(&shapes);
	assert!(
		!all.iter().any(|t| t.contains('▸') || t.contains('▾')),
		"no glyph chevron: {all:?}"
	);
	let (open, height, centre) = chevron(&shapes).unwrap_or_else(|| panic!("no painted chevron"));
	assert!(!open, "starts closed");
	let label = shapes
		.iter()
		.find_map(|s| match &s.shape {
			egui::epaint::Shape::Text(t) if t.galley.text() == "2 worktrees" => {
				let row = t.galley.rows.first()?;
				let g = row.row.glyphs.first()?;
				Some((t.pos.y + row.pos.y + g.pos.y, g.font_ascent))
			}
			_ => None,
		})
		.unwrap_or_else(|| panic!("no label: {all:?}"));
	let (baseline, ascent) = label;
	assert!(
		height >= 0.45 * crate::theme::TEXT_SMALL,
		"big enough to read: {height}"
	);
	assert!(
		centre < baseline && centre > baseline - ascent,
		"centred in the text's body: centre {centre}, baseline {baseline}, ascent {ascent}"
	);
}

#[test]
fn the_chevron_opens_and_closes_the_worktree_rows_and_keeps_the_selection() {
	let ctx = egui::Context::default();
	let mut state = with_worktrees();
	let (before, open) = open_state(&ctx, &mut state);
	assert!(before.iter().any(|t| t == "2 worktrees"), "{before:?}");
	assert_eq!(open, Some(false), "reads closed");
	assert!(
		!before.iter().any(|t| t == "alpha-wt"),
		"starts collapsed: {before:?}"
	);
	let selected = state.selected.clone();
	assert!(selected.is_some(), "the first row is selected");

	click_text(&ctx, &mut state, "2 worktrees");
	let (open_texts, open) = open_state(&ctx, &mut state);
	assert_eq!(open, Some(true), "reads open");
	assert!(open_texts.iter().any(|t| t == "alpha-wt"), "{open_texts:?}");
	assert!(open_texts.iter().any(|t| t == "beta-wt"), "{open_texts:?}");
	assert_eq!(state.selected, selected, "the chevron is not a row click");

	click_text(&ctx, &mut state, "2 worktrees");
	let (closed, open) = open_state(&ctx, &mut state);
	assert_eq!(open, Some(false), "reads closed again");
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
	let (filtered, open) = open_state(&ctx, &mut state);
	assert!(filtered.iter().any(|t| t == "alpha-wt"), "{filtered:?}");
	assert!(!filtered.iter().any(|t| t == "beta-wt"), "{filtered:?}");
	assert!(!filtered.iter().any(|t| t == "q"), "{filtered:?}");
	assert_eq!(open, Some(true), "reads open: {filtered:?}");

	click_text(&ctx, &mut state, "2 worktrees");
	let (closed, open) = open_state(&ctx, &mut state);
	assert!(!closed.iter().any(|t| t == "alpha-wt"), "{closed:?}");
	assert_eq!(open, Some(false), "reads closed: {closed:?}");

	click_text(&ctx, &mut state, "2 worktrees");
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
	click_text(&ctx, &mut state, "2 worktrees");
	let wt = state.repos[0].worktrees[1].path.clone();
	assert_ne!(state.selected.as_ref(), Some(&wt));
	click_text(&ctx, &mut state, "beta-wt");
	assert_eq!(state.selected, Some(wt));
}
