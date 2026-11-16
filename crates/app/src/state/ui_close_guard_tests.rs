//! audit-web-design-guidelines-1793709090 clause 4: a close request with an unsaved commit draft
//! is cancelled and asks first. Keep editing (here Escape) leaves the window and the draft as
//! they are; Close without committing then closes. With no draft, the close goes through.

use std::time::SystemTime;

use eframe::egui::epaint::ClippedShape;
use eframe::egui::{self, Event, Key, ViewportCommand, ViewportEvent, ViewportId, ViewportInfo};
use repose_core::config::Config;

use super::AppState;
use super::ui_frame::{self, screen_input};
use super::ui_harness::{click, key, repo, text_at, texts};

const TITLE: &str = "Close without committing?";

/// One frame, optionally with the window's close button pressed, returning what it painted and
/// the window commands it sent.
fn run(
	ctx: &egui::Context,
	state: &mut AppState,
	close: bool,
	events: Vec<Event>,
) -> (Vec<ClippedShape>, Vec<ViewportCommand>) {
	let mut input = screen_input(events);
	if close {
		input.viewports.insert(
			ViewportId::ROOT,
			ViewportInfo {
				events: vec![ViewportEvent::Close],
				..ViewportInfo::default()
			},
		);
	}
	let out = ui_frame::run(ctx, state, input);
	let commands = out
		.viewport_output
		.get(&ViewportId::ROOT)
		.map(|o| o.commands.clone())
		.unwrap_or_default();
	(out.shapes, commands)
}

fn has(commands: &[ViewportCommand], want: &ViewportCommand) -> bool {
	commands.iter().any(|c| c == want)
}

#[test]
fn a_draft_holds_the_close_until_close_without_committing() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let row = repo("drafted", SystemTime::now());
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	run(&ctx, &mut state, false, Vec::new());
	state
		.commit_messages
		.insert(path.clone(), "half a message".into());

	let (_, commands) = run(&ctx, &mut state, true, Vec::new());
	assert!(
		has(&commands, &ViewportCommand::CancelClose),
		"{commands:?}"
	);
	assert!(!has(&commands, &ViewportCommand::Close), "{commands:?}");
	let (shown, _) = run(&ctx, &mut state, false, Vec::new());
	assert!(
		texts(&shown).iter().any(|t| t == TITLE),
		"the question is shown"
	);
	// Design-polish clause 7: each draft is named by its repository's name, then its path.
	// Search from the dialog's title on: the list behind it also paints the name, as its row.
	let all = texts(&shown);
	let title_at = all.iter().position(|t| t == TITLE).unwrap_or(all.len());
	let dialog = &all[title_at..];
	let name_at = dialog.iter().position(|t| t == "drafted");
	let path_at = dialog.iter().position(|t| t == &path.display().to_string());
	assert!(
		name_at.is_some() && path_at.is_some(),
		"it names the repository and gives its path: {all:?}"
	);
	assert!(name_at < path_at, "the name comes first: {all:?}");

	// Keep editing, here by Escape: the window stays and the draft is untouched.
	let (_, commands) = run(&ctx, &mut state, false, key(Key::Escape));
	assert!(!has(&commands, &ViewportCommand::Close), "{commands:?}");
	assert!(state.close_prompt.is_none());
	assert_eq!(
		state.commit_messages.get(&path).map(String::as_str),
		Some("half a message")
	);

	// Ask again, then choose Close without committing.
	run(&ctx, &mut state, true, Vec::new());
	let (shown, _) = run(&ctx, &mut state, false, Vec::new());
	let at = text_at(&shown, "Close without committing")
		.unwrap_or_else(|| panic!("no close button: {:?}", texts(&shown)));
	let (_, commands) = run(&ctx, &mut state, false, click(at));
	assert!(has(&commands, &ViewportCommand::Close), "{commands:?}");
	assert!(state.close_confirmed);

	// Once confirmed, the close the window then sends goes through without asking again
	// (Morticia Info on f20a5a4), even though the draft is still in memory.
	let (_, commands) = run(&ctx, &mut state, true, Vec::new());
	assert!(
		!has(&commands, &ViewportCommand::CancelClose),
		"{commands:?}"
	);
	assert!(state.close_prompt.is_none());
}

#[test]
fn with_no_draft_the_close_goes_through() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let row = repo("clean", SystemTime::now());
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	state.commit_messages.insert(path, "   \n".into());
	let (_, commands) = run(&ctx, &mut state, true, Vec::new());
	assert!(
		!has(&commands, &ViewportCommand::CancelClose),
		"{commands:?}"
	);
	assert!(state.close_prompt.is_none());
}

const NOTICE_TITLE: &str = "Closed to the notification area";

/// A window with one drafted repository and "Close to tray" on.
fn tray_state(drafted: bool) -> (egui::Context, AppState) {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config {
		close_to_tray: true,
		..Config::default()
	});
	let row = repo("tray", SystemTime::now());
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	run(&ctx, &mut state, false, Vec::new());
	if drafted {
		state.commit_messages.insert(path, "half a message".into());
	}
	(ctx, state)
}

/// close-to-tray clauses 2 and 9: the first close keeps the window up with the notice; OK records
/// it and hides; nothing asks about a draft, since hiding loses nothing.
#[test]
fn the_first_close_to_tray_shows_the_notice_then_hides() {
	let (ctx, mut state) = tray_state(true);
	let (_, commands) = run(&ctx, &mut state, true, Vec::new());
	assert!(
		has(&commands, &ViewportCommand::CancelClose),
		"{commands:?}"
	);
	assert!(
		!has(&commands, &ViewportCommand::Visible(false)),
		"{commands:?}"
	);
	let (shown, _) = run(&ctx, &mut state, false, Vec::new());
	let all = texts(&shown);
	assert!(all.iter().any(|t| t == NOTICE_TITLE), "{all:?}");
	assert!(
		!all.iter().any(|t| t == TITLE),
		"no draft question: {all:?}"
	);
	let ok = text_at(&shown, "OK").unwrap_or_else(|| panic!("no OK: {all:?}"));
	let (_, commands) = run(&ctx, &mut state, false, click(ok));
	assert!(
		has(&commands, &ViewportCommand::Visible(false)),
		"{commands:?}"
	);
	assert!(state.hidden && state.config.tray_notice_shown);
	assert!(!has(&commands, &ViewportCommand::Close), "{commands:?}");
}

/// After the notice, a close hides at once: no notice, no draft question, no close.
#[test]
fn a_later_close_to_tray_hides_at_once() {
	let (ctx, mut state) = tray_state(true);
	state.config.tray_notice_shown = true;
	let (shown, commands) = run(&ctx, &mut state, true, Vec::new());
	assert!(
		has(&commands, &ViewportCommand::CancelClose),
		"{commands:?}"
	);
	assert!(
		has(&commands, &ViewportCommand::Visible(false)),
		"{commands:?}"
	);
	assert!(state.hidden && state.close_prompt.is_none());
	assert!(!texts(&shown).iter().any(|t| t == NOTICE_TITLE));
}

/// close-to-tray clause 4: Quit from the tray goes through the close guard. With a draft while
/// hidden, it shows the window and asks; Keep editing (Escape) cancels the Quit, so the next close
/// hides again. With no draft, it closes.
#[test]
fn quit_from_the_tray_asks_about_a_draft_and_otherwise_closes() {
	let (ctx, mut state) = tray_state(true);
	state.config.tray_notice_shown = true;
	state.hidden = true;
	state.quitting = true;
	let (_, commands) = run(&ctx, &mut state, true, Vec::new());
	assert!(
		has(&commands, &ViewportCommand::CancelClose),
		"{commands:?}"
	);
	assert!(
		has(&commands, &ViewportCommand::Visible(true)),
		"{commands:?}"
	);
	assert!(!state.hidden && state.close_prompt.is_some());
	run(&ctx, &mut state, false, key(Key::Escape));
	assert!(!state.quitting && state.close_prompt.is_none());

	let (ctx, mut state) = tray_state(false);
	state.config.tray_notice_shown = true;
	state.quitting = true;
	let (_, commands) = run(&ctx, &mut state, true, Vec::new());
	assert!(
		!has(&commands, &ViewportCommand::CancelClose),
		"{commands:?}"
	);
}

/// close-to-tray clause 1: with the option off, a close is a close and no window is hidden.
#[test]
fn with_close_to_tray_off_a_close_ends_the_window() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	assert!(!state.config.close_to_tray);
	let (_, commands) = run(&ctx, &mut state, true, Vec::new());
	assert!(
		!has(&commands, &ViewportCommand::CancelClose),
		"{commands:?}"
	);
	assert!(
		!has(&commands, &ViewportCommand::Visible(false)),
		"{commands:?}"
	);
	assert!(!state.hidden);
}
