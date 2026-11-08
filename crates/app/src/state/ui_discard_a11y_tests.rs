//! audit-frontend-1793999566: the discard confirmation's path list is in the AccessKit tree. Each
//! tracked path reads "restore <path>" and each untracked one "remove <path>", as read-only text
//! that is never a Tab stop, so the dialog's only Tab stops stay Cancel and Discard.

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
	for name in ["restore a.txt", "restore src/b.rs", "remove new.txt"] {
		let n = find(&nodes, Role::Label, name);
		assert_eq!(n.name, name, "the node reads the path once, with its verb");
		assert!(!n.focusable, "a path is read, not tabbed to: {n:?}");
	}
	let padded = nodes.iter().filter(|n| n.name.contains("restore  "));
	assert_eq!(
		padded.count(),
		0,
		"no second node carries the padded painted text"
	);
}
