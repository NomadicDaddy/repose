//! The Activity screen's fit (activity-screen clause 8), from the 1440x900 capture at 150% scale
//! (capture slot EXIT #42506): with the pane about 600 points wide, today's cell ends inside the window,
//! and a tile's wrapped text is laid out left-aligned, never justified across the tile.

use eframe::egui::epaint::Shape;
use eframe::egui::{self, Pos2};

use super::activity::{DAYS, day_label, local_today, now_secs};
use super::ui_activity_tests::{fleet, with_reads};
use super::ui_frame::run;

#[test]
fn a_narrow_pane_keeps_today_and_unjustified_tiles() {
	let ctx = egui::Context::default();
	ctx.enable_accesskit();
	let mut state = fleet();
	with_reads(&mut state);
	state.activity.shown = true;
	let width = 920.0;
	let input = || egui::RawInput {
		screen_rect: Some(egui::Rect::from_min_size(
			Pos2::ZERO,
			egui::vec2(width, 600.0),
		)),
		..Default::default()
	};
	run(&ctx, &mut state, input());
	let out = run(&ctx, &mut state, input());

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
	assert!(
		b.x1 <= f64::from(width),
		"today's cell ends at {} in a {width} window",
		b.x1
	);

	let justified: Vec<String> = out
		.shapes
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
