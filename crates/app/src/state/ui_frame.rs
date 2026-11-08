//! The one frame every UI test draws through the real screens (audit-hygiene-1794007485): the
//! left list, then the right pane, with the theme tokens set first as main.rs does. A test that
//! needs more than the painted shapes (the AccessKit tree, the platform's commands, the window's
//! commands) takes the whole output from [`run`], and a test that sets the input's time or a
//! window event builds it from [`screen_input`]. Nothing else in the tests composes a frame. The
//! input events the frames carry (Tab, a key, a click, a wheel turn) are built here too.

use eframe::egui::epaint::ClippedShape;
use eframe::egui::{self, Event, MouseWheelUnit, Pos2, Rect, vec2};

use super::AppState;
use super::ui_harness::{Headless, SCREEN};
use crate::platform::Platform;
use crate::screens::{detail, repo_list};

/// The input of one frame at the tests' screen size, carrying `events`.
pub(super) fn screen_input(events: Vec<Event>) -> egui::RawInput {
	egui::RawInput {
		screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
		events,
		..Default::default()
	}
}

/// One frame of `input` on `platform`, returning all of its output.
pub(super) fn run_on(
	ctx: &egui::Context,
	state: &mut AppState,
	input: egui::RawInput,
	platform: &dyn Platform,
) -> egui::FullOutput {
	let mut out = ctx.run_ui(input, |root| {
		crate::theme::begin_frame(root.ctx());
		repo_list::show(root, state, platform);
		detail::show(root, state, platform);
	});
	// No renderer takes the font texture here; epaint insists the deltas are cleared, not dropped.
	out.textures_delta.clear();
	out
}

/// [`run_on`] on the headless platform, which opens nothing.
pub(super) fn run(
	ctx: &egui::Context,
	state: &mut AppState,
	input: egui::RawInput,
) -> egui::FullOutput {
	run_on(ctx, state, input, &Headless)
}

/// One frame carrying `events`, returning what it painted, for a test that checks what a state
/// looks like rather than what it is.
pub(super) fn frame(
	ctx: &egui::Context,
	state: &mut AppState,
	events: Vec<Event>,
) -> Vec<ClippedShape> {
	run(ctx, state, screen_input(events)).shapes
}

/// [`frame`] on a given platform, for a test that checks what the frame asked it to open.
pub(super) fn frame_on(
	ctx: &egui::Context,
	state: &mut AppState,
	events: Vec<Event>,
	platform: &dyn Platform,
) -> Vec<ClippedShape> {
	run_on(ctx, state, screen_input(events), platform).shapes
}

/// Enough quiet frames (1/60 s each) for a scroll animation to finish.
pub(super) fn settle(ctx: &egui::Context, state: &mut AppState) {
	for _ in 0..40 {
		frame(ctx, state, Vec::new());
	}
}

/// One press and release of Tab, which moves keyboard focus to the next control.
pub(super) fn tab() -> Vec<Event> {
	key(egui::Key::Tab)
}

/// One press and release of `k`.
pub(super) fn key(k: egui::Key) -> Vec<Event> {
	[true, false]
		.into_iter()
		.map(|pressed| Event::Key {
			key: k,
			physical_key: None,
			pressed,
			repeat: false,
			modifiers: egui::Modifiers::default(),
		})
		.collect()
}

/// A primary click at `at`: move, press, release.
pub(super) fn click(at: Pos2) -> Vec<Event> {
	let button = |pressed| Event::PointerButton {
		pos: at,
		button: egui::PointerButton::Primary,
		pressed,
		modifiers: egui::Modifiers::default(),
	};
	vec![Event::PointerMoved(at), button(true), button(false)]
}

/// A wheel turn with the pointer over `at`, enough to move a long list or diff.
pub(super) fn wheel(at: Pos2) -> Vec<Event> {
	vec![
		Event::PointerMoved(at),
		Event::MouseWheel {
			unit: MouseWheelUnit::Point,
			delta: vec2(0.0, -240.0),
			phase: egui::TouchPhase::Move,
			source: egui::MouseWheelSource::Wheel,
			modifiers: egui::Modifiers::default(),
		},
	]
}
