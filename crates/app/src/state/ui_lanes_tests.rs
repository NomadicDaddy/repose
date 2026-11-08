//! branch-graph-lanes clauses 2 and 3 through the real History frame: off by default with the
//! linear view as before, on drawing one dot per commit in its lane with accent connectors where
//! a merge fans out and a fork joins, and the Lanes toggle turning it on for the session.

use std::time::SystemTime;

use eframe::egui::epaint::{ClippedShape, Shape};
use eframe::egui::{self, Pos2};
use repose_core::config::Config;
use repose_core::model::Commit;

use super::ui_harness::{click, frame, repo, text_at, texts};
use super::{AppState, HistoryState, View};
use crate::lanes::layout;
use crate::theme::{ACCENT, SIDEBAR_WIDTH};

fn commit(hash: &str, parents: &[&str], refs: &[&str]) -> Commit {
	Commit {
		hash: hash.repeat(40),
		short: hash.repeat(7),
		subject: format!("commit {hash}"),
		date: "1970-01-01T00:00:00Z".into(),
		parents: parents.iter().map(|p| p.repeat(40)).collect(),
		refs: refs.iter().map(|r| r.to_string()).collect(),
		..Commit::default()
	}
}

/// History on a repository where `side` forked from b, main moved on with a, and m merged side.
fn merged(lanes: bool) -> AppState {
	let mut state = AppState::new(Config::default());
	let row = repo("lanes", SystemTime::now());
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	state.view = View::History;
	state.lanes = lanes;
	let commits = vec![
		commit("m", &["a", "s"], &["main"]),
		commit("a", &["b"], &[]),
		commit("s", &["b"], &["side"]),
		commit("b", &[], &["tag: v1"]),
	];
	let graph = if lanes { layout(&commits) } else { Vec::new() };
	let history = HistoryState {
		commits,
		graph,
		exhausted: true,
		..HistoryState::default()
	};
	state.history.insert(path, history);
	state
}

/// The commit dots drawn in the right pane.
fn dots(shapes: &[ClippedShape]) -> Vec<Pos2> {
	fn walk(shape: &Shape, out: &mut Vec<Pos2>) {
		match shape {
			Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
			Shape::Circle(c) if c.center.x > SIDEBAR_WIDTH => out.push(c.center),
			_ => {}
		}
	}
	let mut out = Vec::new();
	shapes.iter().for_each(|c| walk(&c.shape, &mut out));
	out
}

/// The accent line segments whose ends sit in different columns: the connectors.
fn connectors(shapes: &[ClippedShape]) -> usize {
	fn walk(shape: &Shape) -> usize {
		match shape {
			Shape::Vec(inner) => inner.iter().map(walk).sum(),
			Shape::LineSegment { points, stroke } => {
				usize::from(stroke.color == ACCENT.get() && points[0].x != points[1].x)
			}
			_ => 0,
		}
	}
	shapes.iter().map(|c| walk(&c.shape)).sum()
}

#[test]
fn lanes_are_off_by_default_and_the_linear_view_draws_no_graph() {
	assert!(!AppState::new(Config::default()).lanes, "off at start");
	let ctx = egui::Context::default();
	let mut state = merged(false);
	let shapes = frame(&ctx, &mut state, Vec::new());
	let shown = texts(&shapes);
	assert!(shown.iter().any(|t| t == "commit m"), "{shown:?}");
	assert!(dots(&shapes).is_empty(), "no commit dots");
	assert_eq!(connectors(&shapes), 0, "no connectors");
	assert!(
		!shown.iter().any(|t| t == "side"),
		"no ref pills: {shown:?}"
	);
}

/// Clause 3: two lanes that meet at the merge commit and again where the side branch forked.
#[test]
fn lanes_draw_a_dot_per_commit_in_two_columns_joined_at_the_merge_and_the_fork() {
	let ctx = egui::Context::default();
	let mut state = merged(true);
	let shapes = frame(&ctx, &mut state, Vec::new());
	let at = dots(&shapes);
	assert_eq!(at.len(), 4, "one dot per commit: {at:?}");
	let mut columns: Vec<i32> = at.iter().map(|p| p.x.round() as i32).collect();
	columns.sort_unstable();
	columns.dedup();
	assert_eq!(columns.len(), 2, "two lanes: {at:?}");
	assert!(at[2].x > at[0].x, "the side commit sits in the second lane");
	assert_eq!(
		at[0].x, at[3].x,
		"the merge and the fork point share main's lane"
	);
	assert_eq!(
		connectors(&shapes),
		2,
		"the merge's fan-out and the fork's join"
	);
	let shown = texts(&shapes);
	for pill in ["main", "side", "tag: v1"] {
		assert!(shown.iter().any(|t| t == pill), "{pill} pill: {shown:?}");
	}
}

/// Clause 2: the toggle turns lanes on for the session and re-reads the loaded history.
#[test]
fn the_lanes_toggle_turns_lanes_on_and_rereads_the_history() {
	let ctx = egui::Context::default();
	let mut state = merged(false);
	let shapes = frame(&ctx, &mut state, Vec::new());
	let Some(toggle) = text_at(&shapes, "Lanes") else {
		panic!("no Lanes toggle in {:?}", texts(&shapes));
	};
	let before = state.history.values().map(|h| h.generation).sum::<u64>();
	frame(&ctx, &mut state, click(toggle));
	assert!(state.lanes, "on after the click");
	let after = state.history.values().map(|h| h.generation).sum::<u64>();
	assert_eq!(
		after,
		before + 1,
		"the loaded history re-reads in the new scope"
	);
}

/// Morticia L-1 on 88118fb: in the lanes view a screen reader hears a commit's branch and tag
/// names with the rest of its row; the linear view's names are as before.
#[test]
fn a_commit_row_names_its_refs_only_in_the_lanes_view() {
	use eframe::egui::accesskit::Role;

	use super::ui_a11y_tests::{a11y_frame, find};

	let ctx = egui::Context::default();
	let mut lanes = merged(true);
	frame(&ctx, &mut lanes, Vec::new());
	let nodes = a11y_frame(&ctx, &mut lanes);
	let m = find(&nodes, Role::Button, "mmmmmmm commit m");
	assert!(m.name.ends_with(", main"), "{}", m.name);
	let b = find(&nodes, Role::Button, "bbbbbbb commit b");
	assert!(b.name.ends_with(", tag: v1"), "{}", b.name);

	let ctx = egui::Context::default();
	let mut linear = merged(false);
	frame(&ctx, &mut linear, Vec::new());
	let nodes = a11y_frame(&ctx, &mut linear);
	let m = find(&nodes, Role::Button, "mmmmmmm commit m");
	assert!(!m.name.contains("main"), "{}", m.name);
}
