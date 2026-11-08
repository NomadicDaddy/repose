//! audit-web-design-guidelines-1794001253 (row clipping): a change row's name and folder, and a
//! branch row's name and hash, are cut with an ellipsis where they would run off the row or
//! under its pills, as repository rows and commit subjects already are, through the real frame.

use std::time::SystemTime;

use eframe::egui;
use eframe::egui::epaint::{ClippedShape, Shape};
use repose_core::config::Config;
use repose_core::model::{Branch, Branches, HeadState};

use super::ui_harness::{SCREEN, frame, repo};
use super::{AppState, BranchesState, View};

/// The painted text starting with `start`, and its left and right edges.
fn painted(shapes: &[ClippedShape], start: &str) -> Option<(String, f32, f32)> {
	fn walk(shape: &Shape, start: &str) -> Option<(String, f32, f32)> {
		match shape {
			Shape::Vec(inner) => inner.iter().find_map(|s| walk(s, start)),
			Shape::Text(t) if t.galley.text().starts_with(start) => Some((
				t.galley.text().to_string(),
				t.pos.x,
				t.pos.x + t.galley.size().x,
			)),
			_ => None,
		}
	}
	shapes.iter().find_map(|c| walk(&c.shape, start))
}

const LONG: &str = "a_file_name_long_enough_to_run_far_past_the_edge_of_any_pane_repose_draws_\
	this_wide_and_then_some_more_and_more_and_more.rs";

#[test]
fn a_long_change_row_name_is_cut_with_an_ellipsis_before_the_kind_letter() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let mut row = repo("clip-changes", SystemTime::now());
	if let Some(c) = row.changes.as_mut() {
		c.unstaged[0].path = format!("src/{LONG}{LONG}");
	}
	state.selected = Some(row.path.clone());
	state.merge_discovered(vec![row]);
	state.view = View::Changes;
	let shapes = frame(&ctx, &mut state, Vec::new());
	let Some((text, _, right)) = painted(&shapes, "a_file_name_long") else {
		panic!("the change row's name was not painted");
	};
	assert!(text.ends_with('…'), "cut with an ellipsis: {text:?}");
	assert!(text.chars().count() < 2 * LONG.chars().count());
	// The kind letter's slot is the last 30 px of the row, which ends at the window's edge.
	assert!(
		right <= SCREEN.x - 30.0 + 0.5,
		"the name stops before the letter: {right}"
	);
}

#[test]
fn a_long_branch_name_is_cut_with_an_ellipsis_before_its_pills() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let row = repo("clip-branches", SystemTime::now());
	let path = row.path.clone();
	state.selected = Some(path.clone());
	state.merge_discovered(vec![row]);
	state.view = View::Branches;
	let name = format!("feature/{LONG}");
	state.branches.insert(
		path,
		BranchesState {
			branches: Some(Branches {
				head: HeadState::OnBranch(name.clone()),
				list: vec![Branch {
					name,
					short_hash: "0123abc".into(),
					upstream: None,
					ahead: Some(3),
					behind: None,
					current: true,
				}],
			}),
			..BranchesState::default()
		},
	);
	let shapes = frame(&ctx, &mut state, Vec::new());
	let Some((text, _, name_right)) = painted(&shapes, "feature/a_file") else {
		panic!("the branch name was not painted");
	};
	assert!(text.ends_with('…'), "cut with an ellipsis: {text:?}");
	let Some((_, hash_left, hash_right)) = painted(&shapes, "0123abc") else {
		panic!("the hash was not painted whole");
	};
	assert!(name_right <= hash_left, "the name stops before the hash");
	for pill in ["↑3", "current"] {
		let Some((_, pill_left, _)) = painted(&shapes, pill) else {
			panic!("no {pill} pill");
		};
		assert!(
			hash_right <= pill_left,
			"the hash stops before the {pill} pill"
		);
	}
}

/// Roger and Morticia on 78779f5: a selected change row shows its actions all the time, so its
/// name and folder stop at the actions' left edge, not under the buttons.
#[test]
fn a_selected_change_row_keeps_its_text_clear_of_the_actions() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let mut row = repo("clip-selected", SystemTime::now());
	let long = format!("src/{LONG}{LONG}");
	if let Some(c) = row.changes.as_mut() {
		c.unstaged[0].path = long.clone();
	}
	state.selected = Some(row.path.clone());
	state.merge_discovered(vec![row]);
	state.view = View::Changes;
	state.selected_file = Some(long);
	let shapes = frame(&ctx, &mut state, Vec::new());
	let Some((text, _, right)) = painted(&shapes, "a_file_name_long") else {
		panic!("the change row's name was not painted");
	};
	assert!(text.ends_with('…'), "{text:?}");
	let actions_left = SCREEN.x - crate::screens::changes::ACTIONS_LEFT;
	assert!(
		right <= actions_left,
		"the name ends at {right}, under the actions at {actions_left}"
	);
}

/// Morticia Low 2 on 78779f5: a branch's upstream is cut on one line too, so a long name never
/// leaves it wrapping into a column taller than the row.
#[test]
fn a_branch_upstream_never_wraps_taller_than_its_row() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let row = repo("clip-upstream", SystemTime::now());
	let path = row.path.clone();
	state.selected = Some(path.clone());
	state.merge_discovered(vec![row]);
	state.view = View::Branches;
	// Short enough that the upstream has some room, far too little for its full text.
	let name = "feature/short".to_string();
	state.branches.insert(
		path,
		BranchesState {
			branches: Some(Branches {
				head: HeadState::OnBranch(name.clone()),
				list: vec![Branch {
					name,
					short_hash: "0123abc".into(),
					upstream: Some(format!("origin/{LONG}")),
					ahead: Some(1),
					behind: Some(2),
					current: true,
				}],
			}),
			..BranchesState::default()
		},
	);
	let shapes = frame(&ctx, &mut state, Vec::new());
	// A wrapped galley keeps its text and grows rows, so the rows are what to count.
	fn rows_of(shape: &Shape) -> Option<(String, usize)> {
		match shape {
			Shape::Vec(inner) => inner.iter().find_map(rows_of),
			Shape::Text(t) if t.galley.text().starts_with("origin/") => {
				Some((t.galley.text().to_string(), t.galley.rows.len()))
			}
			_ => None,
		}
	}
	let Some((text, rows)) = shapes.iter().find_map(|c| rows_of(&c.shape)) else {
		panic!("the upstream was not painted");
	};
	assert_eq!(rows, 1, "one line, not wrapped: {text:?}");
	assert!(text.ends_with('…'), "cut with an ellipsis: {text:?}");
}
