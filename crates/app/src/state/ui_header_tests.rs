//! The repository header's remotes lines, drawn through the real right pane: one line per
//! configured remote, the push URL only when it differs, nothing when none is configured, and
//! the failure text when git could not list them (Phillip #31822). Also the upstream line: counts
//! against a tracked upstream, a gone upstream by name, and No upstream only when there is none.

use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;
use repose_core::git::remote::RemoteEntry;

use super::AppState;
use super::ui_harness::{frame, repo, repo_at, texts};

/// Design-polish clause 8: a path too long for the header wraps at a separator, never inside a
/// folder name. The pieces must rejoin into the path, each but the last must end at a
/// separator, and they must sit on more than one line (so a wrap really happened).
#[test]
fn a_long_header_path_wraps_only_at_separators() {
	let mut long = std::env::temp_dir();
	for part in [
		"a-rather-long-workspace-folder",
		"projects-that-belong-to-a-client",
		"the-repository-holding-the-application",
		"nested-checkout-for-a-feature-branch",
		"one-more-level-to-be-sure",
	] {
		long.push(part);
	}
	let mut state = AppState::new(Config::default());
	let row = repo_at(long.clone(), SystemTime::now());
	state.requested_selection = Some(long.clone());
	assert!(state.merge_discovered(vec![row]), "the row is selected");
	let ctx = egui::Context::default();
	crate::theme::apply(&ctx, repose_core::config::Theme::Dark);
	let shapes = frame(&ctx, &mut state, Vec::new());
	let full = long.display().to_string();
	// The pieces a separator-only wrap may cut the path into: each folder with its separator.
	let mut expected: Vec<String> = Vec::new();
	let mut piece = String::new();
	for c in full.chars() {
		piece.push(c);
		if c == '\\' || c == '/' {
			expected.push(std::mem::take(&mut piece));
		}
	}
	expected.push(piece);
	// Texts in the detail pane (right of the 300 px sidebar), below the title: either the whole
	// path in one galley (the old layout) or exactly one of the expected pieces.
	let detail: Vec<(String, f32)> = shapes
		.iter()
		.filter_map(|s| match &s.shape {
			egui::epaint::Shape::Text(t) if t.pos.x > 300.0 => {
				Some((t.galley.text().to_string(), t.pos.y))
			}
			_ => None,
		})
		.collect();
	let title_y = detail
		.iter()
		.find(|(t, _)| t == "one-more-level-to-be-sure")
		.map(|(_, y)| *y)
		.unwrap_or_else(|| panic!("no detail title: {detail:?}"));
	let pieces: Vec<(String, f32)> = detail
		.into_iter()
		.filter(|(t, y)| *y > title_y && (*t == full || expected.contains(t)))
		.collect();
	let joined: String = pieces.iter().map(|(t, _)| t.as_str()).collect();
	assert_eq!(joined, full, "the pieces rejoin into the path: {pieces:?}");
	assert!(
		pieces.len() > 1,
		"the path is laid out in pieces, not one galley: {pieces:?}"
	);
	for (text, _) in &pieces[..pieces.len() - 1] {
		assert!(
			text.ends_with('\\') || text.ends_with('/'),
			"each piece ends at a separator: {pieces:?}"
		);
	}
	let mut lines: Vec<i32> = pieces.iter().map(|(_, y)| *y as i32).collect();
	lines.dedup();
	assert!(
		lines.len() > 1,
		"the path really wraps here, onto {} lines: {pieces:?}",
		lines.len()
	);
	// The wrapped lines sit as close as lines of text, not a widget row apart: the 1.1.1
	// captures at --scale 1.5 showed them about 58 px apart (clause 8, found in clause 9).
	let piece_height = shapes
		.iter()
		.find_map(|s| match &s.shape {
			egui::epaint::Shape::Text(t) if t.galley.text() == pieces[0].0 => {
				Some(t.galley.size().y)
			}
			_ => None,
		})
		.unwrap_or_else(|| panic!("no piece galley"));
	let step = (lines[1] - lines[0]) as f32;
	assert!(
		step <= piece_height * 1.25,
		"wrapped lines {step} px apart for {piece_height} px text"
	);
}

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

#[test]
fn the_upstream_line_names_a_gone_upstream_instead_of_saying_none() {
	let (mut state, _) = selected_state();
	let set = |state: &mut AppState, upstream: Option<&str>, counts: Option<(u32, u32)>| {
		let status = state.repos[0]
			.status
			.as_mut()
			.unwrap_or_else(|| panic!("the fixture row has a status"));
		status.upstream = upstream.map(str::to_string);
		status.ahead = counts.map(|c| c.0);
		status.behind = counts.map(|c| c.1);
	};
	set(&mut state, Some("origin/main"), Some((2, 1)));
	let lines = header_lines(&mut state);
	assert!(
		lines
			.iter()
			.any(|l| l == "↑2 ↓1 against origin/main (saved refs)"),
		"counted upstream: {lines:?}"
	);
	set(&mut state, Some("origin/gone"), None);
	let lines = header_lines(&mut state);
	assert!(
		lines.iter().any(|l| l == "origin/gone · upstream gone"),
		"gone upstream by name: {lines:?}"
	);
	assert!(!lines.iter().any(|l| l == "No upstream"), "{lines:?}");
	set(&mut state, None, None);
	let lines = header_lines(&mut state);
	assert!(
		lines.iter().any(|l| l == "No upstream"),
		"no upstream at all: {lines:?}"
	);
}
