//! The Activity screen (activity-screen), driven through the real frame: start-up with no
//! selection opens on it and no list row is drawn selected; the header button shows it and a row
//! click leaves it; the tiles count from the statuses and filter the grid; and the grid's keyboard
//! moves its own cell, not the repository selection, and Enter opens History.

use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui::{self, Role};
use repose_core::config::Config;
use repose_core::git::activity::{Activity, CommitDay};

use super::activity::{ActivityRead, Tile, local_today, now_secs};
use super::ui_a11y_tests::{a11y_frame, find};
use super::ui_frame::{run, screen_input};
use super::ui_harness::{click, frame, key, repo, text_at, texts};
use super::{AppState, View};

const TITLE: &str = "Activity across all repositories";

/// Three repositories; `b` is ahead of its upstream, and every one has one changed file.
pub(super) fn fleet() -> AppState {
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	let mut b = repo("b", t0);
	if let Some(s) = b.status.as_mut() {
		s.upstream = Some("origin/main".into());
		s.ahead = Some(2);
		s.behind = Some(0);
	}
	state.merge_discovered(vec![repo("a", t0), b, repo("c", t0)]);
	state
}

pub(super) fn path_of(state: &AppState, name: &str) -> PathBuf {
	let found = state.repos.iter().find(|r| r.name == name);
	found.map_or_else(|| panic!("no repository {name}"), |r| r.path.clone())
}

/// `name`'s activity read as `count` commits made `days_ago` local days back (UTC, as the tests
/// set no offset), or none at all for a count of 0.
pub(super) fn read_as(state: &mut AppState, name: &str, count: usize, days_ago: i64) {
	let time = now_secs() - days_ago * 86_400;
	let commit = CommitDay {
		time,
		local_day: local_today(time, 0),
	};
	let commits = vec![commit; count];
	let newest = commits.first().map(|c| c.time);
	let read = ActivityRead::Read(Activity {
		commits,
		newest,
		truncated: false,
	});
	state.activity.reads.insert(path_of(state, name), read);
}

/// Activity read for every repository: `a` committed today, `b` yesterday, `c` never.
pub(super) fn with_reads(state: &mut AppState) {
	read_as(state, "a", 1, 0);
	read_as(state, "b", 1, 1);
	read_as(state, "c", 0, 0);
}

fn shown(ctx: &egui::Context, state: &mut AppState) -> Vec<String> {
	texts(&frame(ctx, state, Vec::new()))
}

/// Clause 2: no --select and no repository at the working folder opens on Activity, and the
/// first row, selected underneath as discovery always does, is not drawn selected.
#[test]
fn start_up_with_no_selection_opens_on_activity() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	state.start_on(None);
	let t0 = SystemTime::now();
	state.merge_discovered(vec![repo("a", t0), repo("b", t0)]);
	let painted = shown(&ctx, &mut state);
	assert!(painted.iter().any(|t| t == TITLE), "{painted:?}");
	assert!(state.activity.shown);

	let mut asked = AppState::new(Config::default());
	asked.start_on(Some(PathBuf::from("D:/x")));
	assert!(
		!asked.activity.shown,
		"a requested repository opens as before"
	);
}

/// Clause 1: the header's Activity button shows the screen; choosing a repository leaves it.
#[test]
fn the_button_shows_activity_and_a_row_click_leaves_it() {
	let ctx = egui::Context::default();
	let mut state = fleet();
	let shapes = frame(&ctx, &mut state, Vec::new());
	let Some(at) = text_at(&shapes, "Activity") else {
		panic!("no Activity button: {:?}", texts(&shapes));
	};
	frame(&ctx, &mut state, click(at));
	assert!(state.activity.shown);
	assert!(shown(&ctx, &mut state).iter().any(|t| t == TITLE));

	let shapes = frame(&ctx, &mut state, Vec::new());
	let Some(row) = text_at(&shapes, "c") else {
		panic!("row c not painted");
	};
	frame(&ctx, &mut state, click(row));
	assert!(!state.activity.shown, "a row click opens that repository");
	assert_eq!(
		state.selected.as_deref(),
		Some(path_of(&state, "c").as_path())
	);
}

/// Clauses 3 and 5: the tiles count from the statuses and reads; a tile filters the grid's rows
/// to its own, and Clear shows them all again. The quiet repository folds into the closing row.
#[test]
fn a_tile_filters_the_grid_to_its_repositories() {
	let ctx = egui::Context::default();
	let mut state = fleet();
	with_reads(&mut state);
	state.activity.shown = true;
	let painted = shown(&ctx, &mut state);
	for label in [
		"Uncommitted changes",
		"Ahead of upstream",
		"▸ 1 quiet repository",
	] {
		assert!(painted.iter().any(|t| t == label), "{label}: {painted:?}");
	}
	assert!(
		painted.iter().any(|t| t == "3"),
		"three uncommitted: {painted:?}"
	);

	let shapes = frame(&ctx, &mut state, Vec::new());
	let Some(tile) = text_at(&shapes, "Ahead of upstream") else {
		panic!("no Ahead tile");
	};
	frame(&ctx, &mut state, click(tile));
	let painted = shown(&ctx, &mut state);
	assert_eq!(state.activity.filter, Some(Tile::Ahead));
	let names: Vec<&String> = painted
		.iter()
		.filter(|t| ["a", "b", "c"].contains(&t.as_str()))
		.collect();
	// The list still paints all three; the grid adds b alone.
	assert_eq!(
		names.iter().filter(|t| t.as_str() == "b").count(),
		2,
		"{names:?}"
	);
	assert_eq!(
		names.iter().filter(|t| t.as_str() == "a").count(),
		1,
		"{names:?}"
	);

	// The same tile again shows them all.
	let shapes = frame(&ctx, &mut state, Vec::new());
	let Some(tile) = text_at(&shapes, "Ahead of upstream") else {
		panic!("no Ahead tile");
	};
	frame(&ctx, &mut state, click(tile));
	assert!(state.activity.filter.is_none());
}

/// activity-all-repositories-card, clauses 1 to 6: the All card comes first with the repository
/// count and is lit while no filter is on; it clears each of the four filters, by click and by
/// keyboard; clicking it while lit changes nothing; and no Showing or Clear line is drawn.
#[test]
fn the_all_card_returns_from_every_filter() {
	let ctx = egui::Context::default();
	let mut state = fleet();
	with_reads(&mut state);
	state.activity.shown = true;
	let shapes = frame(&ctx, &mut state, Vec::new());
	let (Some(all), Some(first)) = (
		text_at(&shapes, "All repositories"),
		text_at(&shapes, "Uncommitted changes"),
	) else {
		panic!("cards not painted: {:?}", texts(&shapes));
	};
	assert!(all.x < first.x, "the All card comes first");
	let painted = texts(&shapes);
	assert!(painted.iter().any(|t| t == "Every listed repository"));
	assert!(
		!painted
			.iter()
			.any(|t| t == "Clear" || t.starts_with("Showing"))
	);
	assert_eq!(
		all_card_selected(&ctx, &mut state),
		Some(true),
		"lit on open"
	);

	frame(&ctx, &mut state, click(all));
	assert!(
		state.activity.filter.is_none(),
		"clicking it lit changes nothing"
	);

	for (label, tile) in [
		("Uncommitted changes", Tile::Uncommitted),
		("Ahead of upstream", Tile::Ahead),
		("Behind upstream", Tile::Behind),
		("Quiet for 60+ days", Tile::Quiet),
	] {
		let shapes = frame(&ctx, &mut state, Vec::new());
		let Some(at) = text_at(&shapes, label) else {
			panic!("no {label}");
		};
		frame(&ctx, &mut state, click(at));
		assert_eq!(state.activity.filter, Some(tile), "{label}");
		let unlit = all_card_selected(&ctx, &mut state);
		assert_eq!(unlit, Some(false), "{label}: All unlit");
		let shapes = frame(&ctx, &mut state, Vec::new());
		let Some(all) = text_at(&shapes, "All repositories") else {
			panic!("no All card under {label}");
		};
		frame(&ctx, &mut state, click(all));
		assert!(state.activity.filter.is_none(), "{label}: All clears it");
	}

	// By keyboard: Tab to the All card, then Enter, and Space for a second filter.
	for (tile, k) in [
		(Tile::Behind, egui::Key::Enter),
		(Tile::Quiet, egui::Key::Space),
	] {
		state.activity.filter = Some(tile);
		tab_to_all_card(&ctx, &mut state);
		frame(&ctx, &mut state, key(k));
		assert!(
			state.activity.filter.is_none(),
			"{k:?} on All clears {tile:?}"
		);
	}
}

/// Whether the All card's node reads as selected, from one frame's AccessKit tree.
fn all_card_selected(ctx: &egui::Context, state: &mut AppState) -> Option<bool> {
	let nodes = a11y_frame(ctx, state);
	find(&nodes, Role::Button, "All repositories: ").selected
}

/// Presses Tab until AccessKit reports the All card focused.
fn tab_to_all_card(ctx: &egui::Context, state: &mut AppState) {
	ctx.enable_accesskit();
	for _ in 0..40 {
		let out = run(ctx, state, screen_input(key(egui::Key::Tab)));
		let Some(update) = out.platform_output.accesskit_update else {
			continue;
		};
		let focused = update.nodes.iter().find(|(id, _)| *id == update.focus);
		if focused.is_some_and(|(_, n)| {
			n.label()
				.is_some_and(|l| l.starts_with("All repositories: "))
		}) {
			return;
		}
	}
	panic!("Tab never reached the All card");
}

/// Clause 7: the grid's arrows move its own cell and leave the repository selection alone; Enter
/// opens the focused row's repository on History.
#[test]
fn the_grid_keyboard_moves_its_cell_and_enter_opens_history() {
	let ctx = egui::Context::default();
	let mut state = fleet();
	with_reads(&mut state);
	state.activity.shown = true;
	frame(&ctx, &mut state, Vec::new());
	let Some(id) = state.activity.grid_id else {
		panic!("the grid was not drawn");
	};
	ctx.memory_mut(|m| m.request_focus(id));
	frame(&ctx, &mut state, Vec::new());
	let before = state.selected.clone();
	state.activity.cell = (0, Some(29));
	frame(&ctx, &mut state, key(egui::Key::ArrowDown));
	frame(&ctx, &mut state, key(egui::Key::ArrowLeft));
	assert_eq!(state.activity.cell, (1, Some(28)));
	assert!(state.activity.shown, "the arrows stayed in the grid");
	assert_eq!(
		state.selected, before,
		"and did not move the list's selection"
	);

	frame(&ctx, &mut state, key(egui::Key::Enter));
	assert!(!state.activity.shown);
	assert_eq!(state.view, View::History);
	// Rows run newest first: a (today), then b (yesterday).
	assert_eq!(
		state.selected.as_deref(),
		Some(path_of(&state, "b").as_path())
	);
}
