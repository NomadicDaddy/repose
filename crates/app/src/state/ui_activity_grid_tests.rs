//! The Activity grid (activity-screen clauses 4, 6, 7 and 9), driven through the real frame and
//! read back from the AccessKit tree: rows newest first, every row name and cell a named node, the
//! quiet repositories opened into the grid by the closing row or by the Quiet tile, Enter on a row
//! name and a click on a cell opening History, the five colour steps as painted, and a read cut
//! at its bound shown as such.

use eframe::egui::epaint::{ClippedShape, Shape};
use eframe::egui::{self, Color32, Pos2, Role};
use repose_core::git::activity::MAX_WALK;

use super::activity::{ActivityRead, DAYS, Tile, day_label, local_today, now_secs};
use super::ui_a11y_tests::{A11yNode, a11y_frame, find};
use super::ui_activity_tests::{fleet, path_of, read_as, with_reads};
use super::ui_frame::{run, screen_input};
use super::ui_harness::{click, frame, key, text_at, texts};
use super::{AppState, View};
use crate::theme::{ACCENT, BG_PANEL};

fn shown_fleet() -> AppState {
	let mut state = fleet();
	with_reads(&mut state);
	state.activity.shown = true;
	state
}

fn of_role(nodes: &[A11yNode], role: Role) -> Vec<&str> {
	nodes
		.iter()
		.filter(|n| n.role == role)
		.map(|n| n.name.as_str())
		.collect()
}

/// Where the node named exactly `name` sits on screen, from one frame's AccessKit tree.
fn centre_of(ctx: &egui::Context, state: &mut AppState, name: &str) -> Pos2 {
	ctx.enable_accesskit();
	let out = run(ctx, state, screen_input(Vec::new()));
	let nodes = out
		.platform_output
		.accesskit_update
		.map(|u| u.nodes)
		.unwrap_or_default();
	let found = nodes
		.iter()
		.find(|(_, n)| n.label().is_some_and(|l| l == name));
	let Some(b) = found.and_then(|(_, n)| n.bounds()) else {
		panic!("no node named {name:?} with bounds");
	};
	egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

/// Clauses 4 and 6: rows run newest first, each row name and every cell is a node named as its
/// tooltip reads, and the quiet repository is folded until its closing row is activated.
#[test]
fn rows_run_newest_first_and_every_cell_is_named() {
	let ctx = egui::Context::default();
	let mut state = shown_fleet();
	frame(&ctx, &mut state, Vec::new());
	let nodes = a11y_frame(&ctx, &mut state);
	let headers = of_role(&nodes, Role::RowHeader);
	assert_eq!(headers.len(), 2, "c is folded: {headers:?}");
	assert_eq!(of_role(&nodes, Role::Cell).len(), 2 * DAYS);
	let today = day_label(DAYS - 1, local_today(now_secs(), 0)).0;
	let a = find(&nodes, Role::Cell, &format!("a, {today}, 1 commit"))
		.name
		.clone();
	let b = find(&nodes, Role::Cell, &format!("b, {today}, 0 commits"))
		.name
		.clone();
	// The tree's order is not the drawing order; the rows' places on screen are.
	let (ya, yb) = (
		centre_of(&ctx, &mut state, &a).y,
		centre_of(&ctx, &mut state, &b).y,
	);
	assert!(ya < yb, "a (today) above b (yesterday): {ya} {yb}");

	let shapes = frame(&ctx, &mut state, Vec::new());
	let Some(fold) = text_at(&shapes, "▸ 1 quiet repository") else {
		panic!("no closing row: {:?}", texts(&shapes));
	};
	frame(&ctx, &mut state, click(fold));
	let nodes = a11y_frame(&ctx, &mut state);
	assert_eq!(
		of_role(&nodes, Role::RowHeader).len(),
		3,
		"opened into the grid"
	);
	assert_eq!(of_role(&nodes, Role::Cell).len(), 3 * DAYS);
	let painted = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(
		painted.iter().any(|t| t == "▾ 1 quiet repository"),
		"{painted:?}"
	);
}

/// Roger 5Q-1 on 4051085: the Quiet tile shows its repositories as grid rows with cells, not an
/// empty grid beside a folded list.
#[test]
fn the_quiet_tile_shows_its_repositories_as_rows() {
	let ctx = egui::Context::default();
	let mut state = fleet();
	read_as(&mut state, "a", 1, 0);
	read_as(&mut state, "b", 1, 70);
	read_as(&mut state, "c", 0, 0);
	state.activity.shown = true;
	state.activity.filter = Some(Tile::Quiet);
	frame(&ctx, &mut state, Vec::new());
	let nodes = a11y_frame(&ctx, &mut state);
	let headers = of_role(&nodes, Role::RowHeader);
	assert_eq!(headers.len(), 2, "b and c: {headers:?}");
	assert_eq!(of_role(&nodes, Role::Cell).len(), 2 * DAYS);
	let painted = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(
		!painted.iter().any(|t| t.contains("quiet repositor")),
		"no fold: {painted:?}"
	);
}

/// Clause 7: Left from a row's first day reaches its name; Enter there opens that repository's
/// History. A click on a cell does the same for its row.
#[test]
fn enter_on_a_row_name_and_a_click_on_a_cell_open_history() {
	let ctx = egui::Context::default();
	let mut state = shown_fleet();
	frame(&ctx, &mut state, Vec::new());
	let Some(id) = state.activity.grid_id else {
		panic!("the grid was not drawn");
	};
	ctx.memory_mut(|m| m.request_focus(id));
	frame(&ctx, &mut state, Vec::new());
	state.activity.cell = (0, Some(0));
	frame(&ctx, &mut state, key(egui::Key::ArrowLeft));
	assert_eq!(state.activity.cell, (0, None));
	frame(&ctx, &mut state, key(egui::Key::Enter));
	assert_eq!(state.view, View::History);
	assert_eq!(
		state.selected.as_deref(),
		Some(path_of(&state, "a").as_path())
	);

	state.activity.shown = true;
	state.view = View::Changes;
	let today = day_label(DAYS - 1, local_today(now_secs(), 0)).0;
	let at = centre_of(&ctx, &mut state, &format!("b, {today}, 0 commits"));
	frame(&ctx, &mut state, click(at));
	assert!(!state.activity.shown);
	assert_eq!(state.view, View::History);
	assert_eq!(
		state.selected.as_deref(),
		Some(path_of(&state, "b").as_path())
	);
}

/// Clauses 4 and 9: the cells are painted in the five steps. 20 commits today is the top step and 5
/// yesterday the middle one; each fill appears once in its cell and once in the legend, and the
/// steps no cell reaches appear in the legend alone.
#[test]
fn cells_are_painted_in_their_colour_steps() {
	let ctx = egui::Context::default();
	let mut state = fleet();
	read_as(&mut state, "a", 20, 0);
	read_as(&mut state, "b", 5, 1);
	read_as(&mut state, "c", 0, 0);
	state.activity.shown = true;
	let shapes = frame(&ctx, &mut state, Vec::new());
	let fills = |shapes: &[ClippedShape], colour: Color32| {
		let rects = shapes.iter().filter_map(|s| match &s.shape {
			Shape::Rect(r) => Some(r.fill),
			_ => None,
		});
		rects.filter(|f| *f == colour).count()
	};
	let step = |t: f32| BG_PANEL.get().lerp_to_gamma(ACCENT.get(), t);
	assert_eq!(fills(&shapes, step(1.0)), 2, "a's day and the legend");
	assert_eq!(fills(&shapes, step(0.5)), 2, "b's day and the legend");
	assert_eq!(fills(&shapes, step(0.25)), 1, "the legend alone");
	assert_eq!(fills(&shapes, step(0.75)), 1, "the legend alone");
}

/// Roger 5Q-1 and Morticia on 4431bdb: a read that stopped at the walk's bound says so on its row,
/// in paint and in its row name, and its cells read as a floor, never as a complete count. Roger
/// 5Q-1 on 5c2b7f0: the painted row keeps its branch beside the note, whole inside the name column.
#[test]
fn a_truncated_read_is_shown_as_a_floor() {
	let ctx = egui::Context::default();
	let mut state = shown_fleet();
	if let Some(ActivityRead::Read(a)) = state.activity.reads.get_mut(&path_of(&state, "a")) {
		a.truncated = true;
	}
	let line = format!("main · 0123abc · newest {MAX_WALK} only");
	let shapes = frame(&ctx, &mut state, Vec::new());
	let painted = shapes.iter().find_map(|s| match &s.shape {
		Shape::Text(t) if t.galley.text() == line => Some((t.visual_bounding_rect(), s.clip_rect)),
		_ => None,
	});
	let Some((bounds, column)) = painted else {
		panic!("{line:?} not painted: {:?}", texts(&shapes));
	};
	assert!(column.contains_rect(bounds), "{bounds:?} cut by {column:?}");
	// A failed read keeps its branch too.
	let failed = ActivityRead::Failed("unreadable".into());
	state.activity.reads.insert(path_of(&state, "c"), failed);
	let painted = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(
		painted
			.iter()
			.any(|t| t == "main · 0123abc · could not read"),
		"{painted:?}"
	);
	let note = format!("newest {MAX_WALK} commits only");
	let nodes = a11y_frame(&ctx, &mut state);
	let today = day_label(DAYS - 1, local_today(now_secs(), 0)).0;
	find(
		&nodes,
		Role::Cell,
		&format!("a, {today}, at least 1 commit"),
	);
	find(&nodes, Role::Cell, &format!("b, {today}, 0 commits"));
	let header = find(&nodes, Role::RowHeader, "a, ");
	assert!(header.name.contains(&note), "{header:?}");
}
