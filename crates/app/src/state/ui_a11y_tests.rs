//! accessible-rendered-content clause 4, checked on the AccessKit tree a frame produces: the
//! painted rows, the worktree chevron, a group count and diff lines each have a node with the
//! right role and a name built from what the row shows; clickable rows stay focusable, and the
//! read-only text (counts, diff lines, separators) is in the tree without being a Tab stop.

use std::time::SystemTime;

use eframe::egui::{self, Pos2, Rect, Role};
use repose_core::config::Config;
use repose_core::git::parse::parse_unified;
use repose_core::model::{ChangeKind, Commit, DiffSource, FileChange};

use super::diff::hash_bytes;
use super::ui_harness::{Headless, SCREEN, frame, real_repo, repo, repo_at};
use super::{
	AppState, DiffContent, DiffState, FileContent, FileRev, FileViewState, HistoryState, OpenDiff,
	OpenFile, View,
};
use crate::diff_rows::build_rows;
use crate::screens::{detail, repo_list};

/// One node of the AccessKit tree a frame produced: its role, its name (label, or value for a
/// label node), whether it is selected, and whether keyboard focus can reach it.
#[derive(Debug)]
pub(super) struct A11yNode {
	pub(super) role: Role,
	pub(super) name: String,
	selected: Option<bool>,
	pub(super) focusable: bool,
	/// The node announces changes politely (a status line).
	polite: bool,
}

/// One frame with AccessKit on, returning every node in the tree update it produced.
pub(super) fn a11y_frame(ctx: &egui::Context, state: &mut AppState) -> Vec<A11yNode> {
	use egui::accesskit::{Action, Toggled};
	ctx.enable_accesskit();
	let input = egui::RawInput {
		screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
		..Default::default()
	};
	let mut out = ctx.run_ui(input, |root| {
		// As main.rs does: the tokens follow the theme this frame draws.
		crate::theme::begin_frame(root.ctx());
		repo_list::show(root, state, &Headless);
		detail::show(root, state, &Headless);
	});
	out.textures_delta.clear();
	let Some(update) = out.platform_output.accesskit_update else {
		panic!("AccessKit was enabled but the frame produced no tree update");
	};
	update
		.nodes
		.into_iter()
		.map(|(_, node)| A11yNode {
			role: node.role(),
			name: node
				.label()
				.or(node.value())
				.unwrap_or_default()
				.to_string(),
			selected: node.toggled().map(|t| t == Toggled::True),
			focusable: node.supports_action(Action::Focus),
			polite: node.live() == Some(egui::accesskit::Live::Polite),
		})
		.collect()
}

pub(super) fn find<'a>(nodes: &'a [A11yNode], role: Role, starts: &str) -> &'a A11yNode {
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
		..Commit::default()
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

#[test]
fn status_lines_are_polite_live_regions() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let mut row = repo("pending", SystemTime::now());
	row.status = None;
	state.merge_discovered(vec![row]);
	frame(&ctx, &mut state, Vec::new());
	let nodes = a11y_frame(&ctx, &mut state);
	let list = find(&nodes, Role::Status, "1 repositor");
	assert!(
		list.polite,
		"the scan and checked line is a polite live region: {list:?}"
	);
	let reading = find(&nodes, Role::Status, "Reading status…");
	assert!(reading.polite, "{reading:?}");
	assert!(
		!list.focusable && !reading.focusable,
		"status lines are not Tab stops"
	);
}

/// Morticia L-1 on 3e371ab: a backstop pass that moves only the clock must not re-announce the
/// status line, so the live region leaves the clock out while the visible line keeps it.
#[test]
fn a_clock_only_change_leaves_the_live_status_unchanged() {
	use std::time::Duration;
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	state.merge_discovered(vec![repo("clocked", SystemTime::now())]);
	let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
	state.checked_at = Some(t0);
	frame(&ctx, &mut state, Vec::new());
	let before = a11y_frame(&ctx, &mut state);
	let live = find(&before, Role::Status, "1 repositor").name.clone();
	assert!(
		!live.contains("Checked"),
		"the live text has no clock: {live:?}"
	);
	let shown = super::ui_harness::texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(
		shown.iter().any(|t| t.contains("Checked")),
		"the screen still shows it"
	);
	state.checked_at = Some(t0 + Duration::from_secs(120));
	let after = a11y_frame(&ctx, &mut state);
	assert_eq!(find(&after, Role::Status, "1 repositor").name, live);
}
