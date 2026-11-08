//! visual-theme clause 5, checked on what a frame paints: Tab reaches the repository rows and the
//! worktree chevrons, whichever holds keyboard focus shows a one-pixel accent outline, and the
//! selected, hovered and focused rows each draw differently from the other two.

use std::time::SystemTime;

use eframe::egui::{self, Color32, Event, Rect, Stroke};
use repose_core::config::Config;

use super::AppState;
use super::ui_harness::{accent_outlines, frame, repo, row_rects, tab};
use crate::theme::{ACCENT, ACCENT_BG, ACCENT_LINE, BG_HOVER, ROW_HEIGHT};

fn with(
	rects: &[(Rect, Color32, Stroke)],
	fill: impl Into<Color32>,
	line: impl Into<Color32>,
) -> Option<Rect> {
	let (fill, line) = (fill.into(), line.into());
	rects
		.iter()
		.find(|(_, f, s)| {
			*f == fill && s.color == line && (s.width == 1.0 || line == Color32::TRANSPARENT)
		})
		.map(|(r, _, _)| *r)
}

#[test]
fn a_focused_row_draws_an_accent_outline_distinct_from_selection_and_hover() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	let rows: Vec<_> = (0..3).map(|i| repo(&format!("f{i}"), t0)).collect();
	state.merge_discovered(rows);
	assert!(
		state.selected.is_some(),
		"the first row is selected by default"
	);
	// Nothing has focus: the selected row has the accent fill and its soft line, and no row
	// shows the bright outline.
	let drawn = row_rects(&frame(&ctx, &mut state, Vec::new()));
	let selected = with(&drawn, ACCENT_BG, ACCENT_LINE)
		.unwrap_or_else(|| panic!("no selected row with the accent fill and soft line: {drawn:?}"));
	assert!(
		drawn.iter().all(|(_, _, s)| s.color != ACCENT.get()),
		"no row shows the focus outline before Tab: {drawn:?}"
	);
	// Tab walks the Refresh button and the Find box, then reaches the first row, which is also
	// the selected one: the accent fill stays and the outline turns bright.
	let mut both = None;
	for _ in 0..6 {
		let drawn = row_rects(&frame(&ctx, &mut state, tab()));
		if let Some(r) = with(&drawn, ACCENT_BG, ACCENT) {
			both = Some(r);
			break;
		}
	}
	let both = both.unwrap_or_else(|| panic!("Tab did not reach the selected row in six presses"));
	assert_eq!(both, selected, "the focused row is the selected one");
	// One more Tab: focus moves to the second row, which draws the outline alone, while the
	// selected row goes back to its soft line.
	let drawn = row_rects(&frame(&ctx, &mut state, tab()));
	let focused = with(&drawn, Color32::TRANSPARENT, ACCENT)
		.unwrap_or_else(|| panic!("no focused, unselected row after the next Tab: {drawn:?}"));
	assert!(focused.top() > selected.top(), "focus moved down the list");
	assert_eq!(
		with(&drawn, ACCENT_BG, ACCENT_LINE),
		Some(selected),
		"the selected row keeps its own look while another row has focus"
	);
	// The pointer over the third row: a grey fill with no line, and the other two unchanged.
	let pitch = focused.top() - selected.top();
	let over_third = focused.center() + egui::vec2(0.0, pitch);
	let drawn = row_rects(&frame(
		&ctx,
		&mut state,
		vec![Event::PointerMoved(over_third)],
	));
	let hovered = with(&drawn, BG_HOVER, Color32::TRANSPARENT)
		.unwrap_or_else(|| panic!("no hovered row under the pointer: {drawn:?}"));
	assert!(hovered.contains(over_third));
	assert_eq!(with(&drawn, Color32::TRANSPARENT, ACCENT), Some(focused));
	assert_eq!(with(&drawn, ACCENT_BG, ACCENT_LINE), Some(selected));
}

/// The worktree chevron is in the Tab order too (Morticia on 7dfa781): after the parent row, the
/// next Tab moves the outline from the row to the chevron's smaller box inside it, and no row
/// carries the outline while the chevron holds focus.
#[test]
fn tab_reaches_the_worktree_chevron_and_outlines_it() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	let mut parent = repo("p", t0);
	parent.worktrees.push(repo("p-wt", t0));
	state.merge_discovered(vec![parent, repo("q", t0)]);
	let mut on_row = false;
	for _ in 0..6 {
		let drawn = frame(&ctx, &mut state, tab());
		if with(&row_rects(&drawn), ACCENT_BG, ACCENT).is_some() {
			on_row = true;
			break;
		}
	}
	assert!(on_row, "Tab did not reach the parent row in six presses");
	let drawn = frame(&ctx, &mut state, tab());
	let rows = row_rects(&drawn);
	let parent_rect = with(&rows, ACCENT_BG, ACCENT_LINE)
		.unwrap_or_else(|| panic!("the parent row keeps its soft line: {rows:?}"));
	assert!(
		rows.iter().all(|(_, _, s)| s.color != ACCENT.get()),
		"no row holds focus while the chevron does: {rows:?}"
	);
	let outlines = accent_outlines(&drawn);
	assert_eq!(outlines.len(), 1, "one focus outline: {outlines:?}");
	let chevron = outlines[0];
	assert!(
		chevron.height() < ROW_HEIGHT && parent_rect.contains_rect(chevron),
		"the outline is the chevron's box inside the parent row: {chevron:?} in {parent_rect:?}"
	);
}
