//! The Activity screen's fit (activity-screen clause 8), from the native captures: with the pane
//! about 600 points wide (1440x900 at 150% scale, slot EXIT #42506) today's cell ends inside the
//! window and a tile's wrapped text is left-aligned, never justified; and when the grid scrolls its
//! bar keeps a lane of its own instead of covering today's column (slot EXIT #42532).

use eframe::egui::epaint::Shape;
use eframe::egui::{self, Pos2, Rect};

use super::AppState;
use super::activity::{DAYS, day_label, local_today, now_secs};
use super::ui_activity_tests::{fleet, with_reads};
use super::ui_frame::run;

fn shown_fleet() -> AppState {
	let mut state = fleet();
	with_reads(&mut state);
	state.activity.shown = true;
	state
}

/// Where repository a's cell for today sits, from a frame's AccessKit tree.
fn todays_cell(out: egui::FullOutput) -> (Rect, Vec<egui::epaint::ClippedShape>) {
	let today = day_label(DAYS - 1, local_today(now_secs(), 0)).0;
	let name = format!("a, {today}, 1 commit");
	let nodes = out
		.platform_output
		.accesskit_update
		.map(|u| u.nodes)
		.unwrap_or_default();
	let found = nodes
		.iter()
		.find(|(_, n)| n.label().is_some_and(|l| l == name));
	let Some(b) = found.and_then(|(_, n)| n.bounds()) else {
		panic!("no node named {name:?}");
	};
	let min = egui::pos2(b.x0 as f32, b.y0 as f32);
	(
		Rect::from_min_max(min, egui::pos2(b.x1 as f32, b.y1 as f32)),
		out.shapes,
	)
}

#[test]
fn a_narrow_pane_keeps_today_and_unjustified_tiles() {
	let ctx = egui::Context::default();
	ctx.enable_accesskit();
	let mut state = shown_fleet();
	let width = 920.0;
	let input = || egui::RawInput {
		screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 600.0))),
		..Default::default()
	};
	run(&ctx, &mut state, input());
	let (cell, shapes) = todays_cell(run(&ctx, &mut state, input()));
	assert!(
		cell.max.x <= width,
		"today's cell ends at {} in a {width} window",
		cell.max.x
	);

	let justified: Vec<String> = shapes
		.iter()
		.filter_map(|s| match &s.shape {
			Shape::Text(t) if t.galley.job.justify => Some(t.galley.text().to_string()),
			_ => None,
		})
		.collect();
	assert!(
		!justified.iter().any(|t| t.contains("Uncommitted")),
		"{justified:?}"
	);
}

/// activity-all-repositories-card clause 7: at widths where the cards' labels and notes wrap to
/// different line counts, the five cards still come out the same height (1440x900 capture at
/// 07b268e); and after the pane widens again they shrink back rather than keep the narrow height.
#[test]
fn the_five_cards_share_one_height_when_labels_wrap() {
	let ctx = egui::Context::default();
	ctx.enable_accesskit();
	let mut state = shown_fleet();
	let mut last = Vec::new();
	for width in [760.0, 1000.0, 1100.0] {
		let heights = card_heights(&ctx, &mut state, width);
		let (low, high) = heights
			.iter()
			.fold((f64::MAX, 0.0_f64), |(lo, hi), h| (lo.min(*h), hi.max(*h)));
		assert!(high - low < 0.5, "at {width}: card heights {heights:?}");
		last = heights;
	}
	// Unequal by nature at 1100 (92 to 107 points without the shared height), and below 760's 176.
	assert!(
		last[0] < 120.0,
		"the cards shrank back after widening: {last:?}"
	);
}

/// The five cards' heights from AccessKit, after frames at `width` settle the shared height.
fn card_heights(ctx: &egui::Context, state: &mut AppState, width: f32) -> Vec<f64> {
	let input = || egui::RawInput {
		screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 600.0))),
		..Default::default()
	};
	run(ctx, state, input());
	run(ctx, state, input());
	let out = run(ctx, state, input());
	let nodes = out
		.platform_output
		.accesskit_update
		.map(|u| u.nodes)
		.unwrap_or_default();
	let cards = [
		"All repositories: ",
		"Uncommitted changes: ",
		"Ahead of upstream: ",
		"Behind upstream: ",
		"Quiet for 60+ days: ",
	];
	cards
		.iter()
		.map(|starts| {
			let found = nodes
				.iter()
				.find(|(_, n)| n.label().is_some_and(|l| l.starts_with(starts)));
			let Some(b) = found.and_then(|(_, n)| n.bounds()) else {
				panic!("no card {starts:?}");
			};
			b.y1 - b.y0
		})
		.collect()
}

/// No painted rectangle (the bar's track or handle) crosses today's cell without containing it.
#[test]
fn the_scroll_bar_never_covers_todays_column() {
	let ctx = egui::Context::default();
	ctx.enable_accesskit();
	let mut state = shown_fleet();
	state.activity.quiet_open = true;
	// The pointer rests over the pane, as the capture's cursor did: a floating bar is drawn only
	// while the pointer is over its area, so without it no bar would be painted to find.
	let input = || egui::RawInput {
		screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(920.0, 330.0))),
		events: vec![egui::Event::PointerMoved(egui::pos2(600.0, 250.0))],
		..Default::default()
	};
	// The floating bar fades in over a few frames; enough of them for it to be fully drawn.
	for _ in 0..30 {
		run(&ctx, &mut state, input());
	}
	let (cell, shapes) = todays_cell(run(&ctx, &mut state, input()));
	let crossing: Vec<Rect> = shapes
		.iter()
		.filter_map(|s| match &s.shape {
			Shape::Rect(r) => Some(r.rect),
			_ => None,
		})
		.filter(|r| r.intersects(cell) && !r.contains_rect(cell) && !cell.contains_rect(*r))
		.collect();
	assert!(
		crossing.is_empty(),
		"today's cell {cell:?} crossed by {crossing:?}"
	);
}
