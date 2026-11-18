//! When Activity's loading glyphs hold still or hold rows (activity-loading-glyphs, from the
//! reviews of 23fdba0): a row that read as quiet stays in the grid with its glyphs until all fade,
//! and folds after (Roger 5Q-5); until Windows' animation setting has been read the glyphs stand
//! still, ask for no frames and swap at once, and move once it is read (Roger 5Q-2); and after
//! 30 s of falling they stand still (Morticia, note 1).

use std::time::Duration;

use eframe::egui;

use super::{at, cells, font_landed, glyph_rows, glyphs, loading};
use crate::state::AppState;
use crate::state::activity::{ActivityRead, GlyphPhase};
use crate::state::ui_activity_tests::{fleet, path_of, read_as};
use crate::state::ui_harness::texts;
use crate::theme::mark_motion_known;

fn chars(shapes: &[egui::epaint::ClippedShape]) -> Vec<char> {
	glyphs(shapes).iter().map(|g| g.1).collect()
}

/// 5Q-5: c reads as quiet (no commits) while a still reads. It is drawn as a grid row holding its
/// glyphs, not folded into the closing row, and folds only after the fade.
#[test]
fn a_quiet_row_holds_its_glyphs_until_all_fade_then_folds() {
	let ctx = egui::Context::default();
	mark_motion_known(&ctx);
	let mut state = fleet();
	read_as(&mut state, "b", 1, 1);
	read_as(&mut state, "c", 0, 0);
	let a = path_of(&state, "a");
	state.activity.reads.insert(a, ActivityRead::Loading);
	state.activity.shown = true;
	at(&ctx, &mut state, 1.0);
	let (shapes, _) = at(&ctx, &mut state, 1.05);
	assert_eq!(glyph_rows(&shapes), 3, "c is a row with glyphs");
	let painted = texts(&shapes);
	assert!(
		!painted.iter().any(|t| t.contains("quiet repositor")),
		"nothing folded yet: {painted:?}"
	);

	read_as(&mut state, "a", 2, 0);
	at(&ctx, &mut state, 2.0);
	let (mid, _) = at(&ctx, &mut state, 2.175);
	assert_eq!(glyph_rows(&mid), 3, "c fades with the others");
	at(&ctx, &mut state, 2.5);
	let (end, _) = at(&ctx, &mut state, 2.6);
	assert_eq!(state.activity.glyphs, GlyphPhase::Done);
	assert_eq!(cells(&end).len(), 2 * 30, "c folded after the fade");
	assert!(texts(&end).iter().any(|t| t == "▸ 1 quiet repository"));
}

/// 5Q-2: before the reduced-motion thread has answered, the glyphs stand still, no frames are
/// asked for, and the last row swaps to boxes at once; once it answers with motion allowed, they
/// move and ask for frames.
#[test]
fn until_the_animation_setting_is_read_the_glyphs_stand_still() {
	let ctx = egui::Context::default();
	let mut state = loading();
	font_landed(&ctx, &mut state, 1.0);
	let (one, _) = at(&ctx, &mut state, 1.05);
	// egui's own short widget animations end within a frame; after that, nothing asks for frames.
	at(&ctx, &mut state, 1.5);
	let (two, delay) = at(&ctx, &mut state, 1.9);
	assert_eq!(glyph_rows(&one), 3);
	assert_eq!(chars(&one), chars(&two), "standing still");
	assert_eq!(delay, Duration::MAX, "no frames asked for");

	mark_motion_known(&ctx);
	let (_, delay) = at(&ctx, &mut state, 2.0);
	let (three, _) = at(&ctx, &mut state, 2.6);
	assert!(delay <= Duration::from_millis(40), "moving now: {delay:?}");
	assert_ne!(chars(&two), chars(&three), "the glyphs cycle");

	let mut unread = loading();
	let ctx = egui::Context::default();
	font_landed(&ctx, &mut unread, 1.0);
	read_as(&mut unread, "a", 2, 0);
	let (end, _) = at(&ctx, &mut unread, 1.1);
	assert_eq!(unread.activity.glyphs, GlyphPhase::Done, "no fade");
	assert!(glyphs(&end).is_empty(), "boxes at once");
}

/// Morticia's note 1: a read that never reports no longer keeps the grid animating; after 30 s of
/// falling the glyphs stand still and no frames are asked for, and the swap is then instant.
#[test]
fn after_thirty_seconds_of_falling_the_glyphs_stand_still() {
	let ctx = egui::Context::default();
	mark_motion_known(&ctx);
	let mut state: AppState = loading();
	font_landed(&ctx, &mut state, 1.0);
	let (_, early) = at(&ctx, &mut state, 20.0);
	assert!(early <= Duration::from_millis(40), "still falling at 19 s");
	at(&ctx, &mut state, 31.5);
	let (one, delay) = at(&ctx, &mut state, 32.0);
	let (two, _) = at(&ctx, &mut state, 40.0);
	assert_eq!(delay, Duration::MAX, "no frames after the cap");
	assert_eq!(chars(&one), chars(&two), "standing still");
	read_as(&mut state, "a", 2, 0);
	at(&ctx, &mut state, 41.0);
	assert_eq!(state.activity.glyphs, GlyphPhase::Done);
}
