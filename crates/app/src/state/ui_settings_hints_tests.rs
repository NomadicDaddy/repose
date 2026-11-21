//! ai-commit-message-style-hints clause 2 through the real settings screen: Match recent subjects
//! starts off, is its own switch beside the provider, can be turned on only once Claude Code is
//! picked, and saves through Save.

use std::fs;
use std::path::PathBuf;

use eframe::egui;
use repose_core::config::{Config, SuggestMessages};

use super::AppState;
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

fn press(ctx: &egui::Context, state: &mut AppState, label: &str) {
	let shown = frame(ctx, state, Vec::new());
	let at = text_at(&shown, label).unwrap_or_else(|| panic!("no {label}: {:?}", texts(&shown)));
	frame(ctx, state, click(at));
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
