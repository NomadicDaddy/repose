//! keyboard-row-focus clause 2 keeps the Space/Enter actions; these tests check them. Every
//! painted row that Tab reaches and AccessKit names a button does on Enter or Space what a click
//! does. A change row opens its diff, a commit row opens or closes, a commit's file row opens
//! that file's diff at the commit, a file-view line becomes the current line, and a repository
//! row becomes the selection. egui supplies the keyboard click for a focused Sense::click widget,
//! and Response::clicked() includes it, so a row that reads clicked_by(Primary) instead would
//! fail here. Each test first Tabs until focus sits on the row it names, checked against the
//! rectangle egui reports for the focused widget, and shows that nothing happened before the key.

use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;
use repose_core::model::{ChangeKind, Commit, CommitFile, DiffSource, FileChange};

use super::ui_harness::{frame, key, repo, tab, text_at};
use super::{AppState, FileContent, FileRev, FileViewState, HistoryState, OpenFile, View};

/// Tabs until the focused widget's rectangle holds where the frame painted `label`, and
/// panics if 60 presses never get there.
fn tab_to(ctx: &egui::Context, state: &mut AppState, label: &str) {
	for _ in 0..60 {
		let shapes = frame(ctx, state, tab());
		let Some(at) = text_at(&shapes, label) else {
			continue;
		};
		let focused = ctx
			.memory(|m| m.focused())
			.and_then(|id| ctx.read_response(id))
			.map(|r| r.rect);
		if focused.is_some_and(|r| r.contains(at)) {
			return;
		}
	}
	panic!("Tab never put focus on the row showing {label:?}");
}

fn files(paths: &[&str]) -> Vec<FileChange> {
	paths
		.iter()
		.map(|p| FileChange {
			path: (*p).into(),
			kind: ChangeKind::Modified,
			from: None,
		})
		.collect()
}

fn changed_repo(state: &mut AppState) -> PathBuf {
	let mut row = repo("activate", SystemTime::now());
	let path = row.path.clone();
	if let Some(changes) = row.changes.as_mut() {
		changes.unstaged = files(&["a.txt", "b.txt"]);
	}
	if let Some(status) = row.status.as_mut() {
		status.unstaged = 2;
	}
	state.merge_discovered(vec![row]);
	path
}

fn commit(n: usize) -> Commit {
	Commit {
		hash: format!("{n:040}"),
		short: format!("{n:07}"),
		author: "test".into(),
		time: 0,
		date: "1970-01-01T00:00:00Z".into(),
		subject: format!("commit {n}"),
		body: String::new(),
	}
}

fn opens_a_change_row(k: egui::Key) {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	changed_repo(&mut state);
	state.view = View::Changes;
	frame(&ctx, &mut state, Vec::new());
	tab_to(&ctx, &mut state, "b.txt");
	assert!(state.diff.is_none(), "focus alone opened a diff");
	frame(&ctx, &mut state, key(k));
	let open = state.diff.as_ref().map(|d| &d.open);
	assert_eq!(open.map(|o| o.display.as_str()), Some("b.txt"), "{k:?}");
	assert_eq!(open.map(|o| &o.source), Some(&DiffSource::Working));
	assert_eq!(state.selected_file.as_deref(), Some("b.txt"));
}

#[test]
fn enter_on_a_focused_change_row_opens_its_diff() {
	opens_a_change_row(egui::Key::Enter);
}

#[test]
fn space_on_a_focused_change_row_opens_its_diff() {
	opens_a_change_row(egui::Key::Space);
}

fn history(state: &mut AppState, expanded: Option<usize>) -> PathBuf {
	let path = changed_repo(state);
	state.view = View::History;
	let commits: Vec<_> = (0..3).map(commit).collect();
	let mut h = HistoryState {
		commits,
		exhausted: true,
		..HistoryState::default()
	};
	if let Some(n) = expanded {
		let hash = commit(n).hash;
		h.files.insert(
			hash.clone(),
			Ok(vec![CommitFile {
				path: "src/lib.rs".into(),
				kind: ChangeKind::Modified,
				from: None,
			}]),
		);
		h.expanded = Some(hash);
	}
	state.history.insert(path.clone(), h);
	path
}

#[test]
fn enter_on_a_focused_commit_row_opens_and_closes_it() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let path = history(&mut state, None);
	frame(&ctx, &mut state, Vec::new());
	tab_to(&ctx, &mut state, "commit 1");
	let expanded = |s: &AppState| s.history.get(&path).and_then(|h| h.expanded.clone());
	assert_eq!(expanded(&state), None, "focus alone opened a commit");
	frame(&ctx, &mut state, key(egui::Key::Enter));
	assert_eq!(expanded(&state), Some(commit(1).hash));
	frame(&ctx, &mut state, key(egui::Key::Enter));
	assert_eq!(
		expanded(&state),
		None,
		"a second Enter closes it, as a second click does"
	);
}

#[test]
fn enter_on_a_focused_commit_file_row_opens_its_diff_at_the_commit() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	history(&mut state, Some(0));
	frame(&ctx, &mut state, Vec::new());
	tab_to(&ctx, &mut state, "src/lib.rs");
	assert!(state.diff.is_none(), "focus alone opened a diff");
	frame(&ctx, &mut state, key(egui::Key::Enter));
	let open = state.diff.as_ref().map(|d| &d.open);
	assert_eq!(open.map(|o| o.display.as_str()), Some("src/lib.rs"));
	assert_eq!(
		open.map(|o| &o.source),
		Some(&DiffSource::Commit(commit(0).hash))
	);
}

#[test]
fn enter_on_a_focused_file_view_line_makes_it_current() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let path = changed_repo(&mut state);
	state.file_view = Some(FileViewState {
		open: OpenFile {
			repo: path,
			rev: FileRev::Head,
			path: "a.txt".into(),
			size: Some(40),
		},
		content: FileContent::Text {
			lines: (0..5).map(|i| format!("line {i}")).collect(),
			colours: Vec::new(),
			columns: 6,
		},
		generation: 0,
		current_line: None,
		notice: None,
	});
	frame(&ctx, &mut state, Vec::new());
	tab_to(&ctx, &mut state, "line 2");
	let current = |s: &AppState| s.file_view.as_ref().and_then(|f| f.current_line);
	assert_eq!(current(&state), None, "focus alone set the current line");
	frame(&ctx, &mut state, key(egui::Key::Enter));
	assert_eq!(
		current(&state),
		Some(3),
		"line 2 is the third line, numbered from 1"
	);
}

#[test]
fn enter_on_a_focused_repository_row_selects_it() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let first = repo("activate-a", SystemTime::now());
	let second = repo("activate-b", SystemTime::now());
	let (a, b) = (first.path.clone(), second.path.clone());
	state.merge_discovered(vec![first, second]);
	state.select(a.clone());
	frame(&ctx, &mut state, Vec::new());
	tab_to(&ctx, &mut state, "activate-b");
	assert_eq!(
		state.selected.as_ref(),
		Some(&a),
		"focus alone moved the selection"
	);
	frame(&ctx, &mut state, key(egui::Key::Enter));
	assert_eq!(state.selected.as_ref(), Some(&b));
}
