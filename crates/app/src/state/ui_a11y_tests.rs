//! accessible-rendered-content clause 4, checked on the AccessKit tree a frame produces: the
//! painted rows, the worktree chevron, a group count and diff lines each have a node with the
//! right role and a name built from what the row shows; clickable rows stay focusable, and the
//! read-only text (counts, diff lines, separators) is in the tree without being a Tab stop.

use std::time::SystemTime;

use eframe::egui::{self, Role};
use repose_core::config::Config;
use repose_core::git::parse::parse_unified;
use repose_core::model::{ChangeKind, Commit, DiffSource, FileChange};

use super::diff::hash_bytes;
use super::ui_harness::{A11yNode, a11y_frame, frame, real_repo, repo, repo_at};
use super::{
	AppState, DiffContent, DiffState, FileContent, FileRev, FileViewState, HistoryState, OpenDiff,
	OpenFile, View,
};
use crate::diff_rows::build_rows;

fn find<'a>(nodes: &'a [A11yNode], role: Role, starts: &str) -> &'a A11yNode {
	nodes
		.iter()
		.find(|n| n.role == role && n.name.starts_with(starts))
		.unwrap_or_else(|| {
			let named: Vec<_> = nodes.iter().map(|n| (n.role, n.name.as_str())).collect();
			panic!("no {role:?} node starting {starts:?} in {named:?}")
		})
}

fn with_three_changes(state: &mut AppState) -> std::path::PathBuf {
	let mut row = repo("named", SystemTime::now());
	if let Some(changes) = row.changes.as_mut() {
		changes.unstaged = ["a.txt", "b.txt", "c.txt"]
			.into_iter()
			.map(|p| FileChange {
				path: p.into(),
				kind: ChangeKind::Modified,
				from: None,
			})
			.collect();
	}
	if let Some(status) = row.status.as_mut() {
		status.unstaged = 3;
	}
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	path
}

#[test]
fn repository_rows_and_the_chevron_are_named_buttons() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	let mut parent = repo("p", t0);
	parent.worktrees.push(repo("p-wt", t0));
	state.merge_discovered(vec![parent, repo("q", t0)]);
	frame(&ctx, &mut state, Vec::new());
	let nodes = a11y_frame(&ctx, &mut state);
	let p = find(&nodes, Role::Button, "p, main at 0123abc, ");
	assert!(p.name.ends_with(", 1 changed"), "{p:?}");
	assert_eq!(p.selected, Some(true), "the first row is selected");
	assert!(p.focusable);
	let q = find(&nodes, Role::Button, "q, main at 0123abc, ");
	assert_eq!(q.selected, Some(false));
	let chevron = nodes
		.iter()
		.find(|n| n.role == Role::DisclosureTriangle)
		.unwrap_or_else(|| panic!("no chevron node: {nodes:?}"));
	assert!(
		chevron.name == "Expand 1 worktree" || chevron.name == "Collapse 1 worktree",
		"{chevron:?}"
	);
	assert!(chevron.focusable);
}

#[test]
fn change_rows_name_path_and_kind_and_the_count_is_text() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	with_three_changes(&mut state);
	state.view = View::Changes;
	state.selected_file = Some("a.txt".into());
	frame(&ctx, &mut state, Vec::new());
	let nodes = a11y_frame(&ctx, &mut state);
	let a = find(&nodes, Role::Button, "a.txt, modified");
	assert_eq!((a.selected, a.focusable), (Some(true), true));
	let b = find(&nodes, Role::Button, "b.txt, modified");
	assert_eq!(b.selected, Some(false));
	let count = find(&nodes, Role::Label, "3 files");
	assert!(!count.focusable, "a count is read, not tabbed to");
}

#[test]
fn commit_rows_and_file_lines_are_named() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let path = with_three_changes(&mut state);
	state.view = View::History;
	let commit = Commit {
		hash: "a".repeat(40),
		short: "aaaaaaa".into(),
		author: "Ada".into(),
		time: 0,
		date: "1970-01-01T00:00:00Z".into(),
		subject: "Name the rows".into(),
		body: String::new(),
	};
	let mut history = HistoryState {
		commits: vec![commit],
		exhausted: true,
		..HistoryState::default()
	};
	history.expanded = Some("a".repeat(40));
	history.files.insert("a".repeat(40), Ok(Vec::new()));
	state.history.insert(path.clone(), history);
	frame(&ctx, &mut state, Vec::new());
	let nodes = a11y_frame(&ctx, &mut state);
	let row = find(&nodes, Role::Button, "aaaaaaa Name the rows, Ada, ");
	assert_eq!(
		(row.selected, row.focusable),
		(Some(true), true),
		"the open commit"
	);

	state.file_view = Some(FileViewState {
		open: OpenFile {
			repo: path,
			rev: FileRev::Head,
			path: "a.txt".into(),
			size: Some(20),
		},
		content: FileContent::Text {
			lines: vec!["first".into(), "second".into()],
			colours: Vec::new(),
			columns: 6,
		},
		generation: 0,
		current_line: Some(2),
		notice: None,
	});
	frame(&ctx, &mut state, Vec::new());
	let nodes = a11y_frame(&ctx, &mut state);
	assert_eq!(
		find(&nodes, Role::Button, "Line 1: first").selected,
		Some(false)
	);
	let second = find(&nodes, Role::Button, "Line 2: second");
	assert_eq!((second.selected, second.focusable), (Some(true), true));
}

#[test]
fn diff_lines_and_separators_are_text_and_never_tab_stops() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let real = real_repo("a11y-diff");
	state.merge_discovered(vec![repo_at(real.clone(), SystemTime::now())]);
	state.selected_file = Some("x.txt".into());
	let text = "diff --git a/x.txt b/x.txt\n--- a/x.txt\n+++ b/x.txt\n\
		@@ -1,1 +1,1 @@\n-old one\n+new one\n@@ -10,1 +10,1 @@ fn main\n-old ten\n+new ten\n";
	let files = parse_unified(text.as_bytes());
	let rows = vec![build_rows(&files[0], false)];
	state.diff = Some(DiffState {
		open: OpenDiff {
			repo: real,
			source: DiffSource::Working,
			paths: vec!["x.txt".into()],
			display: "x.txt".into(),
		},
		content: DiffContent::Files(files),
		inline: false,
		generation: 1,
		rows,
		rows_inline: false,
		colours: Vec::new(),
		raw_hash: Some(hash_bytes(b"")),
	});
	state.diff_serial = 1;
	frame(&ctx, &mut state, Vec::new());
	let nodes = a11y_frame(&ctx, &mut state);
	let first = find(
		&nodes,
		Role::Label,
		"removed line 1: old one; added line 1: new one",
	);
	let tenth = find(
		&nodes,
		Role::Label,
		"removed line 10: old ten; added line 10: new ten",
	);
	let gap = find(&nodes, Role::Heading, "8 unchanged lines, fn main");
	for n in [first, tenth, gap] {
		assert!(!n.focusable, "diff text is never a Tab stop: {n:?}");
	}
}
