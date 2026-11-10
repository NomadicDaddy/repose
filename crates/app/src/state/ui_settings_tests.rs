//! settings-screen clauses 1 to 4 through the real frames: Settings in the list header opens the
//! screen pre-filled from the config; a missing root or a bad command is marked and Save does
//! nothing until it is fixed; Save writes the --config file, keeps a key repose does not know, and
//! rescans with the new roots in the same session; Cancel and Escape drop the edits, and Escape
//! leaves the selection behind the screen where it was.

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui::{self, Key};
use repose_core::config::{self, Config};

use super::AppState;
use super::ui_harness::{click, frame, key, repo, text_at, texts};

/// A folder that exists, to stand as a valid root.
fn real_root(name: &str) -> PathBuf {
	let dir = std::env::temp_dir()
		.join("repose-ui-settings")
		.join(format!("{name}-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("root: {e}"));
	dir
}

/// A config whose one root exists, so the screen opens with nothing marked.
fn state_with_root(name: &str) -> (AppState, PathBuf) {
	let root = real_root(name);
	let state = AppState::new(Config {
		roots: vec![root.clone()],
		..Config::default()
	});
	(state, root)
}

/// Opens the screen and draws its first frame: a Modal paints nothing on the frame it opens.
fn open(ctx: &egui::Context, state: &mut AppState) {
	state.open_settings();
	frame(ctx, state, Vec::new());
}

/// Clicks the painted `label`, failing with what the frame showed when it is not there.
fn press(ctx: &egui::Context, state: &mut AppState, label: &str) {
	let shown = frame(ctx, state, Vec::new());
	let at = text_at(&shown, label).unwrap_or_else(|| panic!("no {label}: {:?}", texts(&shown)));
	frame(ctx, state, click(at));
}

#[test]
fn settings_opens_from_the_header_pre_filled_from_the_config() {
	let ctx = egui::Context::default();
	let (mut state, root) = state_with_root("open");
	press(&ctx, &mut state, "Settings");
	assert!(state.settings.is_some(), "the screen is open");
	// The tests' 400 px window shows the top of the screen and its buttons; the fields below
	// scroll, so the rest of the pre-fill is read from the draft.
	let shown = texts(&frame(&ctx, &mut state, Vec::new()));
	let root_text = root.display().to_string();
	for want in [root_text.as_str(), "node_modules", "Save", "Cancel"] {
		assert!(shown.iter().any(|t| t == want), "{want} missing: {shown:?}");
	}
	let Some(draft) = &state.settings else {
		panic!("closed");
	};
	assert_eq!(draft.roots, [root_text]);
	assert_eq!(draft.exclude, state.config.exclude);
	assert_eq!(
		(draft.depth, draft.theme),
		(state.config.depth, state.config.theme)
	);
	assert_eq!(
		(draft.editor.as_str(), draft.terminal.as_str()),
		("code", "wt")
	);
}

#[test]
fn a_missing_root_or_a_bad_command_is_marked_and_save_waits() {
	let ctx = egui::Context::default();
	let (mut state, _) = state_with_root("marked");
	open(&ctx, &mut state);
	if let Some(draft) = state.settings.as_mut() {
		draft.roots.push("Q:\\no\\such\\root".into());
		draft.editor = "code --wait".into();
	}
	let shown = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(
		shown.iter().any(|t| t == "This folder does not exist."),
		"{shown:?}"
	);
	assert!(super::SettingsDraft::command_problem("code --wait").is_some());
	press(&ctx, &mut state, "Save");
	assert!(state.settings.is_some(), "Save did nothing");
	assert_eq!(
		state.config.commands.editor, "code",
		"the config is unchanged"
	);

	if let Some(draft) = state.settings.as_mut() {
		draft.roots.pop();
		draft.editor = "notepad".into();
	}
	press(&ctx, &mut state, "Save");
	assert!(state.settings.is_none(), "fixed, Save closes the screen");
	assert_eq!(state.config.commands.editor, "notepad");
}

#[test]
fn save_writes_the_config_file_and_rescans_with_the_new_roots() {
	let ctx = egui::Context::default();
	let first = real_root("save-first");
	let second = real_root("save-second");
	let path = first.join("config.toml");
	let text = format!("roots = ['{}']\nfuture_key = 1\n", first.display());
	fs::write(&path, &text).unwrap_or_else(|e| panic!("config: {e}"));
	let mut state = AppState::new(config::load_or_create(&path).unwrap_or_else(|e| panic!("{e}")));
	state.config_path = Some(path.clone());
	state.merge_discovered(vec![repo("alpha", SystemTime::now())]);
	assert!(!state.scanning);

	open(&ctx, &mut state);
	if let Some(draft) = state.settings.as_mut() {
		draft.new_root = second.display().to_string();
	}
	press(&ctx, &mut state, "Add");
	press(&ctx, &mut state, "Save");
	assert!(state.settings.is_none());
	assert_eq!(state.config.roots, [first.clone(), second.clone()]);
	assert!(
		state.scanning,
		"the new roots are scanned without a restart"
	);

	let saved = fs::read_to_string(&path).unwrap_or_default();
	assert!(
		saved.contains("future_key = 1"),
		"an unknown key is kept: {saved}"
	);
	let reloaded = config::load_or_create(&path).unwrap_or_else(|e| panic!("{e}"));
	assert_eq!(reloaded.roots, [first, second]);
}

#[test]
fn cancel_and_escape_drop_the_edits() {
	let ctx = egui::Context::default();
	let (mut state, root) = state_with_root("cancel");
	state.merge_discovered(vec![
		repo("alpha", SystemTime::now()),
		repo("beta", SystemTime::now()),
	]);
	let alpha = state.repos[0].path.clone();
	state.selected = Some(alpha.clone());

	open(&ctx, &mut state);
	if let Some(draft) = state.settings.as_mut() {
		draft.depth = 7;
	}
	press(&ctx, &mut state, "Cancel");
	assert!(state.settings.is_none());
	assert_eq!(state.config.depth, Config::default().depth);

	open(&ctx, &mut state);
	if let Some(draft) = state.settings.as_mut() {
		draft.roots.clear();
	}
	frame(&ctx, &mut state, Vec::new());
	frame(&ctx, &mut state, key(Key::ArrowDown));
	assert_eq!(
		state.selected.as_ref(),
		Some(&alpha),
		"keys stay with the screen"
	);
	frame(&ctx, &mut state, key(Key::Escape));
	assert!(state.settings.is_none(), "Escape closes the screen");
	assert_eq!(state.config.roots, [root], "and drops its edits");
	assert_eq!(state.selected.as_ref(), Some(&alpha));
}
