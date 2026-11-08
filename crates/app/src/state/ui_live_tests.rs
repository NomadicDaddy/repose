//! audit-web-design-guidelines-1794001253 (pane loading labels): each pane's loading text is a
//! polite live region that is not a Tab stop, so a screen reader hears when a pane starts and
//! stops loading. A switch of view starts a real read on a thread; the frame never pumps its
//! result, so the loading text is still up when the tree is read.

use std::time::SystemTime;

use eframe::egui;
use eframe::egui::accesskit::Role;
use repose_core::config::Config;
use repose_core::model::{Commit, DiffSource};

use super::ui_a11y_tests::{A11yNode, a11y_frame, find};
use super::ui_harness::{frame, repo};
use super::{AppState, DiffContent, DiffState, HistoryState, OpenDiff, View};

/// One repository selected, on `view`, through two frames as the app draws them.
fn on(view: View, set: impl FnOnce(&mut AppState, &std::path::Path)) -> Vec<A11yNode> {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let row = repo("loading", SystemTime::now());
	let path = row.path.clone();
	state.selected = Some(path.clone());
	state.merge_discovered(vec![row]);
	state.view = view;
	set(&mut state, &path);
	frame(&ctx, &mut state, Vec::new());
	a11y_frame(&ctx, &mut state)
}

fn assert_live(nodes: &[A11yNode], text: &str) {
	let node = find(nodes, Role::Status, text);
	assert!(node.polite, "{text}: {node:?}");
	assert!(!node.focusable, "{text} is not a Tab stop");
}

#[test]
fn pane_loading_labels_are_polite_live_regions() {
	assert_live(&on(View::History, |_, _| {}), "Reading history…");
	assert_live(&on(View::Branches, |_, _| {}), "Reading branches…");
	assert_live(&on(View::Files, |_, _| {}), "Reading tree…");
	let diff = on(View::Changes, |state, path| {
		state.diff = Some(DiffState {
			open: OpenDiff {
				repo: path.to_path_buf(),
				source: DiffSource::Working,
				paths: vec!["x.txt".into()],
				display: "x.txt".into(),
			},
			content: DiffContent::Loading,
			inline: false,
			generation: 1,
			rows: Vec::new(),
			rows_inline: false,
			colours: Vec::new(),
			raw_hash: None,
		});
	});
	assert_live(&diff, "Reading diff…");
}

/// An expanded commit whose files are still being read, and Load more while a page loads: the
/// button keeps its role and a polite region beside it says "Loading…".
#[test]
fn commit_files_and_load_more_announce_their_loading() {
	let nodes = on(View::History, |state, path| {
		let hash = "c".repeat(40);
		let history = HistoryState {
			commits: vec![Commit {
				hash: hash.clone(),
				short: hash[..7].into(),
				subject: "a commit".into(),
				date: "1970-01-01T00:00:00Z".into(),
				..Commit::default()
			}],
			expanded: Some(hash),
			loading: true,
			..HistoryState::default()
		};
		state.history.insert(path.to_path_buf(), history);
	});
	assert_live(&nodes, "Reading files…");
	assert_live(&nodes, "Loading…");
	find(&nodes, Role::Button, "Loading…");
}

/// Control: an ordinary label is not a live region, so the assertion above can fail.
#[test]
fn a_plain_label_is_not_a_live_region() {
	let nodes = on(View::Changes, |_, _| {});
	let label = find(&nodes, Role::Label, "Find a repository");
	assert!(!label.polite, "the Find label is a plain label: {label:?}");
}
