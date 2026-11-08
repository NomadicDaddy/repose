//! keyboard-row-focus clause 3, checked on what a frame paints: Tab reaches the change rows, the
//! commit rows and the file-view lines, whichever holds keyboard focus shows the one-pixel accent
//! outline every list row uses, and the focused, selected (open, current) and hovered rows each
//! draw differently from the other two.

use std::path::PathBuf;

use eframe::egui::{self, Color32, Event, Rect, Stroke};
use repose_core::config::Config;
use repose_core::model::Commit;

use super::ui_harness::{frame, pane_rects, rect_with as with, tab, with_three_changes};
use super::{AppState, FileContent, FileRev, FileViewState, HistoryState, OpenFile, View};
use crate::theme::{ACCENT, ACCENT_BG, ACCENT_LINE, BG_HOVER};

const CHANGE_ROW: f32 = 30.0;
const COMMIT_ROW: f32 = 32.0;
const FILE_LINE: f32 = 20.0;

type Drawn = Vec<(Rect, Color32, Stroke)>;

/// Tabs until the selected row holds focus (its fill under the bright outline), then on until
/// another row of the same height takes the outline alone; an open commit's own controls may
/// sit between the two. Returns the selected row and the frame where another row holds focus.
fn tab_past_selected(
	ctx: &egui::Context,
	state: &mut AppState,
	height: f32,
	selected_fill: impl Into<Color32> + Copy,
) -> (Rect, Drawn) {
	let mut both = None;
	for _ in 0..40 {
		let drawn = pane_rects(&frame(ctx, state, tab()), height);
		if let Some(r) = with(&drawn, selected_fill, ACCENT) {
			both = Some(r);
			break;
		}
	}
	let both = both.unwrap_or_else(|| panic!("Tab did not reach the selected row"));
	for _ in 0..10 {
		let drawn = pane_rects(&frame(ctx, state, tab()), height);
		if with(&drawn, Color32::TRANSPARENT, ACCENT).is_some() {
			return (both, drawn);
		}
	}
	panic!("Tab did not move focus to another row of height {height}")
}

fn hover(ctx: &egui::Context, state: &mut AppState, at: egui::Pos2, height: f32) -> Drawn {
	pane_rects(&frame(ctx, state, vec![Event::PointerMoved(at)]), height)
}

fn selected_repo(state: &mut AppState) -> PathBuf {
	with_three_changes(state, "focus")
}

#[test]
fn change_rows_show_focus_apart_from_selection_and_hover() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	selected_repo(&mut state);
	state.view = View::Changes;
	state.selected_file = Some("a.txt".into());
	let drawn = pane_rects(&frame(&ctx, &mut state, Vec::new()), CHANGE_ROW);
	let selected = with(&drawn, ACCENT_BG, ACCENT_LINE)
		.unwrap_or_else(|| panic!("no selected change row: {drawn:?}"));
	assert!(
		drawn.iter().all(|(_, _, s)| s.color != ACCENT.get()),
		"no focus before Tab"
	);
	let (both, drawn) = tab_past_selected(&ctx, &mut state, CHANGE_ROW, ACCENT_BG);
	assert_eq!(
		both, selected,
		"the selected row showed both looks while focused"
	);
	let focused = with(&drawn, Color32::TRANSPARENT, ACCENT)
		.unwrap_or_else(|| panic!("no focused, unselected change row: {drawn:?}"));
	assert_eq!(with(&drawn, ACCENT_BG, ACCENT_LINE), Some(selected));
	// Under the commit box only two change rows fit the 400 px window, so the hovered row is the
	// first one with its selection cleared: neither selected nor focused, beside the focused row.
	state.selected_file = None;
	let first = selected.center();
	let drawn = hover(&ctx, &mut state, first, CHANGE_ROW);
	let hovered = with(&drawn, BG_HOVER, Color32::TRANSPARENT)
		.unwrap_or_else(|| panic!("no hovered change row: {drawn:?}"));
	assert!(hovered.contains(first));
	assert_eq!(with(&drawn, Color32::TRANSPARENT, ACCENT), Some(focused));
}

fn commit(n: usize) -> Commit {
	Commit {
		hash: format!("{n:040}"),
		short: format!("{n:07}"),
		author: "test".into(),
		time: 0,
		date: "1970-01-01T00:00:00Z".into(),
		subject: format!("commit {n}"),
		body: String::new(),
		..Commit::default()
	}
}

#[test]
fn commit_rows_show_focus_apart_from_the_open_commit_and_hover() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let path = selected_repo(&mut state);
	state.view = View::History;
	let commits: Vec<_> = (0..3).map(commit).collect();
	let open = commits[0].hash.clone();
	let mut history = HistoryState {
		commits,
		exhausted: true,
		expanded: Some(open.clone()),
		..HistoryState::default()
	};
	history.files.insert(open, Ok(Vec::new()));
	state.history.insert(path, history);
	let drawn = pane_rects(&frame(&ctx, &mut state, Vec::new()), COMMIT_ROW);
	let opened = with(&drawn, ACCENT_BG, ACCENT_LINE)
		.unwrap_or_else(|| panic!("no open commit row: {drawn:?}"));
	assert!(
		drawn.iter().all(|(_, _, s)| s.color != ACCENT.get()),
		"no focus before Tab"
	);
	let (both, drawn) = tab_past_selected(&ctx, &mut state, COMMIT_ROW, ACCENT_BG);
	assert_eq!(
		both, opened,
		"the open commit showed both looks while focused"
	);
	let focused = with(&drawn, Color32::TRANSPARENT, ACCENT)
		.unwrap_or_else(|| panic!("no focused, closed commit row: {drawn:?}"));
	assert!(focused.top() > opened.top(), "focus moved down the list");
	assert_eq!(with(&drawn, ACCENT_BG, ACCENT_LINE), Some(opened));
	let below = egui::pos2(focused.center().x, focused.bottom() + 8.0);
	let drawn = hover(&ctx, &mut state, below, COMMIT_ROW);
	let hovered = with(&drawn, BG_HOVER, Color32::TRANSPARENT)
		.unwrap_or_else(|| panic!("no hovered commit row: {drawn:?}"));
	assert!(hovered.contains(below));
	assert_eq!(with(&drawn, Color32::TRANSPARENT, ACCENT), Some(focused));
}

#[test]
fn file_view_lines_show_focus_apart_from_the_current_line_and_hover() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let path = selected_repo(&mut state);
	state.file_view = Some(FileViewState {
		open: OpenFile {
			repo: path,
			rev: FileRev::Head,
			path: "a.txt".into(),
			size: Some(40),
		},
		content: FileContent::Text {
			lines: (0..5).map(|i| format!("line {i}")).collect(),
			colours: Vec::new(),
			columns: 6,
		},
		generation: 0,
		current_line: Some(1),
		notice: None,
	});
	let drawn = pane_rects(&frame(&ctx, &mut state, Vec::new()), FILE_LINE);
	let current = with(&drawn, ACCENT_BG, Color32::TRANSPARENT)
		.unwrap_or_else(|| panic!("no current line: {drawn:?}"));
	assert!(
		drawn.iter().all(|(_, _, s)| s.color != ACCENT.get()),
		"no focus before Tab"
	);
	let (both, drawn) = tab_past_selected(&ctx, &mut state, FILE_LINE, ACCENT_BG);
	assert_eq!(
		both, current,
		"the current line showed both looks while focused"
	);
	let focused = with(&drawn, Color32::TRANSPARENT, ACCENT)
		.unwrap_or_else(|| panic!("no focused, non-current line: {drawn:?}"));
	assert_eq!(with(&drawn, ACCENT_BG, Color32::TRANSPARENT), Some(current));
	// A file-view line has no hover fill: the line under the pointer stays plain, so it cannot
	// be taken for the current or the focused line.
	let below = focused.center() + egui::vec2(0.0, FILE_LINE);
	let drawn = hover(&ctx, &mut state, below, FILE_LINE);
	assert!(
		drawn.iter().all(|(r, _, _)| !r.contains(below)),
		"the hovered line draws no fill or outline: {drawn:?}"
	);
	assert_eq!(with(&drawn, Color32::TRANSPARENT, ACCENT), Some(focused));
	assert_eq!(with(&drawn, ACCENT_BG, Color32::TRANSPARENT), Some(current));
}
