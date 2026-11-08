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

/// Morticia Low on c505d7a: the painted fold triangle is click-only, so Tab never stops on an
/// unnamed node before a group's header; every focusable node on the Changes view has a name.
#[test]
fn the_fold_triangle_is_not_an_unnamed_tab_stop() {
	use super::ui_a11y_tests::a11y_frame;

	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let row = repo("fold-a11y", SystemTime::now());
	state.selected = Some(row.path.clone());
	state.merge_discovered(vec![row]);
	state.view = View::Changes;
	frame(&ctx, &mut state, Vec::new());
	let nodes = a11y_frame(&ctx, &mut state);
	let unnamed: Vec<_> = nodes
		.iter()
		.filter(|n| n.focusable && n.name.is_empty())
		.map(|n| n.role)
		.collect();
	assert!(unnamed.is_empty(), "unnamed Tab stops: {unnamed:?}");
	let header = nodes
		.iter()
		.find(|n| n.name == "Changes, expanded")
		.unwrap_or_else(|| panic!("no named header for the Changes group"));
	assert!(header.focusable, "the header button is the keyboard path");
}

/// Criterion 4 of the record (Roger on b67cdf8): folding a group and unfolding it leaves the
/// pane's scroll where it was, here with the view scrolled down a long Changes group.
#[test]
fn folding_a_group_keeps_the_scroll_position() {
	use super::ui_harness::settle;

	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let mut row = repo("fold-scroll", SystemTime::now());
	if let Some(c) = row.changes.as_mut() {
		c.staged.push(FileChange {
			path: "staged.txt".into(),
			kind: ChangeKind::Modified,
			from: None,
		});
		for i in 0..40 {
			c.unstaged.push(FileChange {
				path: format!("file{i:02}.txt"),
				kind: ChangeKind::Modified,
				from: None,
			});
		}
	}
	state.selected = Some(row.path.clone());
	state.merge_discovered(vec![row]);
	state.view = View::Changes;
	frame(&ctx, &mut state, Vec::new());
	// A small turn, so the view moves but the Staged header stays on screen to be clicked.
	let small_turn = vec![
		egui::Event::PointerMoved(egui::pos2(800.0, 300.0)),
		egui::Event::MouseWheel {
			unit: egui::MouseWheelUnit::Point,
			delta: egui::vec2(0.0, -60.0),
			phase: egui::TouchPhase::Move,
			source: egui::MouseWheelSource::Wheel,
			modifiers: egui::Modifiers::default(),
		},
	];
	frame(&ctx, &mut state, small_turn);
	settle(&ctx, &mut state);
	let Some(before) = crate::screens::detail::scroll_offset(&ctx) else {
		panic!("the detail pane publishes its offset");
	};
	assert!(before > 0.0, "the view scrolled: {before}");
	for _ in 0..2 {
		let shapes = frame(&ctx, &mut state, Vec::new());
		let Some(header) = text_at(&shapes, "Staged Changes") else {
			panic!(
				"the Staged header is on screen after the scroll: {:?}",
				texts(&shapes)
			);
		};
		frame(&ctx, &mut state, click(header));
		settle(&ctx, &mut state);
	}
	let after = crate::screens::detail::scroll_offset(&ctx);
	assert_eq!(
		after,
		Some(before),
		"folded and unfolded, the scroll is where it was"
	);
	let shown = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(
		shown.iter().any(|t| t == "staged.txt"),
		"unfolded again: {shown:?}"
	);
}
