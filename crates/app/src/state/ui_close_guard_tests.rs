//! audit-web-design-guidelines-1793709090 clause 4: a close request with an unsaved commit draft
//! is cancelled and asks first. Keep editing (here Escape) leaves the window and the draft as
//! they are; Close without committing then closes. With no draft, the close goes through.

use std::time::SystemTime;

use eframe::egui::epaint::ClippedShape;
use eframe::egui::{
	self, Event, Key, Pos2, Rect, ViewportCommand, ViewportEvent, ViewportId, ViewportInfo,
};
use repose_core::config::Config;

use super::AppState;
use super::ui_harness::{Headless, SCREEN, click, key, repo, text_at, texts};
use crate::screens::{detail, repo_list};

const TITLE: &str = "Close without committing?";

/// One frame, optionally with the window's close button pressed, returning what it painted and
/// the window commands it sent.
fn run(
	ctx: &egui::Context,
	state: &mut AppState,
	close: bool,
	events: Vec<Event>,
) -> (Vec<ClippedShape>, Vec<ViewportCommand>) {
	let mut input = egui::RawInput {
		screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
		events,
		..Default::default()
	};
	if close {
		input.viewports.insert(
			ViewportId::ROOT,
			ViewportInfo {
				events: vec![ViewportEvent::Close],
				..ViewportInfo::default()
			},
		);
	}
	let mut out = ctx.run_ui(input, |root| {
		repo_list::show(root, state, &Headless);
		detail::show(root, state, &Headless);
	});
	out.textures_delta.clear();
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
	assert!(
		texts(&shown)
			.iter()
			.any(|t| t == &path.display().to_string()),
		"it names the repository"
	);

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
