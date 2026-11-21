//! ai-commit-message-style-hints clause 2 through the real settings screen: Match recent subjects
//! starts off, is its own switch beside the provider, can be turned on only once Claude Code is
//! picked, and saves through Save.

use std::fs;
use std::path::PathBuf;

use eframe::egui;
use repose_core::config::{Config, SuggestMessages};

use super::AppState;
use super::ui_frame::{settle, wheel};
use super::ui_harness::{click, frame, text_at, texts};

const HINTS: &str = "Match recent subjects";

fn state_with(provider: SuggestMessages) -> AppState {
	let root: PathBuf = std::env::temp_dir()
		.join("repose-ui-settings-hints")
		.join(format!("{}-{:?}", std::process::id(), provider));
	fs::create_dir_all(&root).unwrap_or_else(|e| panic!("root: {e}"));
	AppState::new(Config {
		roots: vec![root],
		suggest_messages: provider,
		..Config::default()
	})
}

/// Opens the screen and draws its first frame: a Modal paints nothing on the frame it opens.
fn open(ctx: &egui::Context, state: &mut AppState) {
	state.open_settings();
	frame(ctx, state, Vec::new());
}

/// Clicks the painted `label`. The screen scrolls, so when the label is below the fold the wheel
/// is turned over the screen first, as a user would, until it is painted.
fn press(ctx: &egui::Context, state: &mut AppState, label: &str) {
	for _ in 0..5 {
		let shown = frame(ctx, state, Vec::new());
		if let Some(at) = text_at(&shown, label) {
			frame(ctx, state, click(at));
			return;
		}
		// Just above Save, which sits under the scrolling fields, is always inside them.
		let save =
			text_at(&shown, "Save").unwrap_or_else(|| panic!("no Save: {:?}", texts(&shown)));
		frame(ctx, state, wheel(save - egui::vec2(0.0, 60.0)));
		settle(ctx, state);
	}
	panic!("no {label} after scrolling");
}

fn draft_hints(state: &AppState) -> bool {
	state
		.settings
		.as_ref()
		.is_some_and(|d| d.suggest_style_hints)
}

#[test]
fn style_hints_wait_for_a_provider_and_start_off() {
	let ctx = egui::Context::default();
	let mut state = state_with(SuggestMessages::Off);
	open(&ctx, &mut state);
	assert!(!draft_hints(&state), "off by default");
	press(&ctx, &mut state, HINTS);
	assert!(
		!draft_hints(&state),
		"not offered while suggestions are off"
	);
}

#[test]
fn style_hints_turn_on_beside_claude_code_and_save() {
	let ctx = egui::Context::default();
	let mut state = state_with(SuggestMessages::Claude);
	open(&ctx, &mut state);
	assert!(!draft_hints(&state), "off by default");
	press(&ctx, &mut state, HINTS);
	assert!(draft_hints(&state), "offered once Claude Code is picked");
	press(&ctx, &mut state, "Save");
	assert!(state.config.suggest_style_hints);
	assert_eq!(state.config.suggest_messages, SuggestMessages::Claude);
}
