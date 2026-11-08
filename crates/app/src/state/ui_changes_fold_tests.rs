//! audit-frontend-1793999566: each Changes group folds to its header and back with a click on
//! the header, through the real frame. The group's count and its Stage all or Unstage all stay on
//! the header while it is folded, the other groups are untouched, and the selected file is kept.

use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;
use repose_core::model::{ChangeKind, FileChange};

use super::ui_harness::{click, frame, repo, text_at, texts};
use super::{AppState, View};

#[test]
fn a_changes_group_folds_to_its_header_and_back() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let mut row = repo("fold", SystemTime::now());
	if let Some(c) = row.changes.as_mut() {
		c.staged.push(FileChange {
			path: "staged.txt".into(),
			kind: ChangeKind::Modified,
			from: None,
		});
	}
	state.selected = Some(row.path.clone());
	state.merge_discovered(vec![row]);
	state.view = View::Changes;
	state.selected_file = Some("x.txt".into());

	let shown = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(shown.iter().any(|t| t == "staged.txt"), "{shown:?}");
	let shapes = frame(&ctx, &mut state, Vec::new());
	let Some(header) = text_at(&shapes, "Staged Changes") else {
		panic!("no Staged Changes header in {:?}", texts(&shapes));
	};
	frame(&ctx, &mut state, click(header));
	let folded = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(
		!folded.iter().any(|t| t == "staged.txt"),
		"folded: {folded:?}"
	);
	for kept in ["Staged Changes", "Unstage all", "x.txt", "Changes"] {
		assert!(folded.iter().any(|t| t == kept), "{kept} stays: {folded:?}");
	}
	assert_eq!(
		state.selected_file.as_deref(),
		Some("x.txt"),
		"the selection is kept"
	);

	let shapes = frame(&ctx, &mut state, Vec::new());
	let Some(header) = text_at(&shapes, "Staged Changes") else {
		panic!("the folded header is still drawn");
	};
	frame(&ctx, &mut state, click(header));
	let open = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(open.iter().any(|t| t == "staged.txt"), "unfolded: {open:?}");
}
