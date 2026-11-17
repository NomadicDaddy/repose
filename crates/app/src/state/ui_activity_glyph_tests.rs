//! Activity's loading glyphs (activity-loading-glyphs), driven through the real frame with the
//! input's time set: glyphs in the cells while any row reads, rows that finished early holding
//! theirs, a fade that starts only when the last row lands, the plain grid after it with no more
//! repaints asked for, a failed row on plain boxes, the "reading" names unchanged, and with
//! animations off still glyphs and an instant swap.

use std::time::Duration;

use eframe::egui::epaint::{ClippedShape, Shape};
use eframe::egui::{self, Pos2, Rect, Role, ViewportId};

use super::AppState;
use super::activity::{ActivityRead, GlyphPhase};
use super::ui_a11y_tests::{a11y_frame, find};
use super::ui_activity_tests::{fleet, path_of, read_as};
use super::ui_frame::{run, screen_input};
use crate::theme::{ACCENT, BG_PANEL, LINE_SOFT, set_motion};

const DIGITS: &str = "0123456789";
const KANA: &str = "ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜﾝ";

/// One frame at `time`: what it painted, and the repaint delay it asked for.
fn at(ctx: &egui::Context, state: &mut AppState, time: f64) -> (Vec<ClippedShape>, Duration) {
	let input = egui::RawInput {
		time: Some(time),
		..screen_input(Vec::new())
	};
	let out = run(ctx, state, input);
	let delay = out
		.viewport_output
		.get(&ViewportId::ROOT)
		.map_or(Duration::MAX, |v| v.repaint_delay);
	(out.shapes, delay)
}

/// The day cells as painted: outlined in LINE_SOFT and a grid row's cell height.
fn cells(shapes: &[ClippedShape]) -> Vec<(Rect, egui::Color32)> {
	shapes
		.iter()
		.filter_map(|s| match &s.shape {
			Shape::Rect(r) if r.stroke.color == LINE_SOFT.get() && r.rect.height() == 28.0 => {
				Some((r.rect, r.fill))
			}
			_ => None,
		})
		.collect()
}

/// The glyphs painted inside day cells: (where, the glyph, the colour it is painted in). The
/// colour is the shape's override, which renders natively; a placeholder's fallback did not.
fn glyphs(shapes: &[ClippedShape]) -> Vec<(Pos2, char, Option<egui::Color32>)> {
	let cells = cells(shapes);
	shapes
		.iter()
		.filter_map(|s| match &s.shape {
			Shape::Text(t) => {
				let mid = t.pos + t.galley.rect.center().to_vec2();
				let mut chars = t.galley.text().chars();
				let (Some(c), None) = (chars.next(), chars.next()) else {
					return None;
				};
				cells.iter().any(|(r, _)| r.contains(mid)).then_some((
					mid,
					c,
					t.override_text_color,
				))
			}
			_ => None,
		})
		.collect()
}

/// a still reading; b and c already read.
fn loading() -> AppState {
	let mut state = fleet();
	read_as(&mut state, "b", 1, 1);
	read_as(&mut state, "c", 3, 2);
	let a = path_of(&state, "a");
	state.activity.reads.insert(a, ActivityRead::Loading);
	state.activity.shown = true;
	state
}

/// Clauses 1 to 3 and 5: every cell of every row carries a glyph while a reads, b's and c's
/// included; the glyphs change over time; the fade starts when a lands, and after it the cells are
/// their real fills with no glyph and no repaint asked for.
#[test]
fn glyphs_fall_while_a_row_reads_and_fade_once_the_last_lands() {
	let ctx = egui::Context::default();
	let mut state = loading();
	at(&ctx, &mut state, 1.0);
	let (shapes, delay) = at(&ctx, &mut state, 1.05);
	assert_eq!(state.activity.glyphs, GlyphPhase::Falling);
	let first = glyphs(&shapes);
	assert_eq!(first.len(), cells(&shapes).len(), "one glyph in every cell");
	assert_eq!(first.len(), 3 * 30, "b and c hold theirs");
	assert!(
		first
			.iter()
			.all(|(_, c, _)| DIGITS.contains(*c) || KANA.contains(*c))
	);
	// Painted in the accent ramp's upper steps (2 to 4), never the text colour.
	let ramp = [0.5, 0.75, 1.0].map(|t| Some(BG_PANEL.get().lerp_to_gamma(ACCENT.get(), t)));
	assert!(
		first.iter().all(|(_, _, colour)| ramp.contains(colour)),
		"{:?}",
		first.iter().map(|g| g.2).collect::<Vec<_>>()
	);
	assert!(
		cells(&shapes).iter().all(|(_, f)| *f == BG_PANEL.get()),
		"no real fill yet"
	);
	assert!(
		delay <= Duration::from_millis(40),
		"falling asks for frames: {delay:?}"
	);
	let (later, _) = at(&ctx, &mut state, 1.6);
	let moved = glyphs(&later)
		.iter()
		.zip(&first)
		.filter(|(a, b)| a.1 != b.1)
		.count();
	assert!(moved > first.len() / 2, "the glyphs cycle: {moved} changed");

	read_as(&mut state, "a", 2, 0);
	let (_, delay) = at(&ctx, &mut state, 2.0);
	assert_eq!(state.activity.glyphs, GlyphPhase::Fading(2.0));
	assert_eq!(delay, Duration::ZERO, "the fade asks for every frame");
	let (mid, _) = at(&ctx, &mut state, 2.175);
	let half = glyphs(&mid);
	assert_eq!(half.len(), 3 * 30, "all rows fade together");
	assert!(
		half.iter()
			.all(|(_, _, c)| c.is_some_and(|c| c.a() > 0 && c.a() < 255)),
		"half faded"
	);

	at(&ctx, &mut state, 2.4);
	assert_eq!(state.activity.glyphs, GlyphPhase::Done);
	let (end, delay) = at(&ctx, &mut state, 2.5);
	assert!(glyphs(&end).is_empty(), "the plain grid");
	assert!(
		cells(&end).iter().any(|(_, f)| *f != BG_PANEL.get()),
		"real fills"
	);
	assert_eq!(delay, Duration::MAX, "no repaint once the fade ends");

	// A later refresh reads every row again; it shows plain boxes, not glyphs.
	let a = path_of(&state, "a");
	state.activity.reads.insert(a, ActivityRead::Loading);
	let (again, _) = at(&ctx, &mut state, 3.0);
	assert!(glyphs(&again).is_empty(), "no glyphs on a later refresh");
}

/// Clause 4: a failed row ends on plain boxes with no glyphs while the others still fall, and a
/// reading row keeps its accessible name.
#[test]
fn a_failed_row_has_plain_boxes_and_names_are_unchanged() {
	let ctx = egui::Context::default();
	let mut state = loading();
	let b = path_of(&state, "b");
	state
		.activity
		.reads
		.insert(b, ActivityRead::Failed("bad".into()));
	at(&ctx, &mut state, 1.0);
	let (shapes, _) = at(&ctx, &mut state, 1.05);
	assert_eq!(glyphs(&shapes).len(), 2 * 30, "only a and c carry glyphs");
	let nodes = a11y_frame(&ctx, &mut state);
	find(&nodes, Role::Cell, "a, reading");
	find(&nodes, Role::Cell, "b, could not read");
}

/// Clause 3: with animations off the glyphs stand still, nothing asks for frames, and the swap to
/// boxes is instant when the last row lands.
#[test]
fn with_animations_off_the_glyphs_stand_still_and_swap_at_once() {
	let ctx = egui::Context::default();
	ctx.all_styles_mut(|s| set_motion(s, false));
	let mut state = loading();
	at(&ctx, &mut state, 1.0);
	let (one, _) = at(&ctx, &mut state, 1.05);
	// The frame after the glyph font lands asks for one more pass to settle; after that, nothing.
	let (two, delay) = at(&ctx, &mut state, 1.9);
	let still: Vec<char> = glyphs(&one).iter().map(|g| g.1).collect();
	assert_eq!(still.len(), 3 * 30);
	assert_eq!(
		still,
		glyphs(&two).iter().map(|g| g.1).collect::<Vec<_>>(),
		"static"
	);
	assert_eq!(delay, Duration::MAX, "no frames asked for");
	read_as(&mut state, "a", 2, 0);
	let (end, _) = at(&ctx, &mut state, 2.0);
	assert_eq!(state.activity.glyphs, GlyphPhase::Done);
	assert!(glyphs(&end).is_empty(), "boxes at once");
}
