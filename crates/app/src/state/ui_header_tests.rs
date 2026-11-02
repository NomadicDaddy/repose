//! The repository header's remotes lines, drawn through the real right pane: one line per
//! configured remote, the push URL only when it differs, nothing when none is configured, and
//! the failure text when git could not list them (Phillip #31822).

use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;
use repose_core::git::remote::RemoteEntry;

use super::AppState;
use super::ui_harness::{frame, repo, texts};

/// A state with one row, selected, and the path of that row.
fn selected_state() -> (AppState, PathBuf) {
	let mut state = AppState::new(Config::default());
	let row = repo("with-remotes", SystemTime::now());
	let path = row.path.clone();
	state.requested_selection = Some(path.clone());
	assert!(state.merge_discovered(vec![row]), "the row is selected");
	assert_eq!(state.selected, Some(path.clone()));
	(state, path)
}

fn header_lines(state: &mut AppState) -> Vec<String> {
	let ctx = egui::Context::default();
	let shapes = frame(&ctx, state, Vec::new());
	texts(&shapes)
}

#[test]
fn the_header_lists_each_configured_remote_with_a_differing_push_url() {
	let (mut state, path) = selected_state();
	state.remotes.insert(
		path,
		Ok(vec![
			RemoteEntry {
				name: "origin".into(),
				url: "https://example.invalid/team/thing.git".into(),
				push_url: None,
			},
			RemoteEntry {
				name: "mirror".into(),
				url: "git@example.invalid:team/thing.git".into(),
				push_url: Some("ssh://push.example.invalid/thing".into()),
			},
			RemoteEntry {
				name: "tokened".into(),
				url: "https://user:ghp_secret@example.invalid/team/thing.git".into(),
				push_url: None,
			},
		]),
	);
	let lines = header_lines(&mut state);
	assert!(
		!lines
			.iter()
			.any(|l| l.contains("ghp_secret") || l.contains("user:")),
		"a token in a remote URL never reaches the header: {lines:?}"
	);
	assert!(
		lines
			.iter()
			.any(|l| l == "tokened · https://example.invalid/team/thing.git"),
		"the tokened remote is shown without its userinfo: {lines:?}"
	);
	assert!(
		lines
			.iter()
			.any(|l| l == "origin · https://example.invalid/team/thing.git"),
		"origin line missing: {lines:?}"
	);
	assert!(
		lines.iter().any(|l| l
			== "mirror · git@example.invalid:team/thing.git (push ssh://push.example.invalid/thing)"),
		"mirror line with its push URL missing: {lines:?}"
	);
}

#[test]
fn no_remote_line_is_drawn_without_a_configured_remote_and_a_failure_is_named() {
	let (mut state, path) = selected_state();
	state.remotes.insert(path.clone(), Ok(Vec::new()));
	let lines = header_lines(&mut state);
	assert!(
		!lines.iter().any(|l| l.contains(" · ") && l.contains("://")),
		"no remote line for an empty list: {lines:?}"
	);
	assert!(
		!lines
			.iter()
			.any(|l| l.starts_with("Remotes could not be read")),
		"an empty list is not a failure: {lines:?}"
	);
	state
		.remotes
		.insert(path, Err("git exited 128: not a git repository".into()));
	let lines = header_lines(&mut state);
	assert!(
		lines
			.iter()
			.any(|l| l == "Remotes could not be read: git exited 128: not a git repository"),
		"the failure is named: {lines:?}"
	);
}
