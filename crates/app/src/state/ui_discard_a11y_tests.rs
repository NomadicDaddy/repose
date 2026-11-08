//! audit-frontend-1793999566: the discard confirmation's path list is in the AccessKit tree. The
//! paths are plain egui labels, and each one's Label node is named with its row's text: the verb
//! padded to nine columns, then the path, so "restore  a.txt" or "remove   new.txt". None is a
//! Tab stop, so Tab still moves only between Cancel and Discard. Measured on de1ee6e's
//! confirm.rs before this test existed: the labels already gave these nodes (Morticia #39647).

use std::time::SystemTime;

use eframe::egui::{self, Role};
use repose_core::config::Config;

use super::ui_a11y_tests::{a11y_frame, find};
use super::ui_harness::{frame, repo};
use super::{AppState, PendingDiscard};

#[test]
fn every_path_in_the_discard_dialog_is_read_and_none_is_a_tab_stop() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let row = repo("a11y-discard", SystemTime::now());
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	state.select(path.clone());
	state.pending_discard = Some(PendingDiscard {
		repo: path,
		tracked: vec!["a.txt".into(), "src/b.rs".into()],
		untracked: vec!["new.txt".into()],
		return_focus: None,
		focus_cancel: true,
	});
	frame(&ctx, &mut state, Vec::new());
	let nodes = a11y_frame(&ctx, &mut state);
	for name in ["restore  a.txt", "restore  src/b.rs", "remove   new.txt"] {
		let n = find(&nodes, Role::Label, name);
		assert_eq!(n.name, name, "the node reads the row's whole text");
		assert!(!n.focusable, "a path is read, not tabbed to: {n:?}");
	}
}
