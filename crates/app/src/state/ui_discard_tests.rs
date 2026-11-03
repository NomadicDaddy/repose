//! Drive the actual discard dialog through painted frames and pointer input.

use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui::epaint::{ClippedShape, Shape};
use eframe::egui::{self, Event, PointerButton};
use repose_core::config::Config;

use super::ui_harness::{frame, real_repo, repo_at, tab, texts};
use super::{AppState, PendingDiscard, WriteKind};

fn prepared(name: &str) -> (AppState, PathBuf) {
	let path = real_repo(name);
	std::fs::write(path.join("x.txt"), "modified\n")
		.unwrap_or_else(|error| panic!("prepare tracked change: {error}"));
	let mut state = AppState::new(Config::default());
	state.repos = vec![repo_at(path.clone(), SystemTime::now())];
	state.selected = Some(path.clone());
	state.pending_discard = Some(PendingDiscard {
		repo: path.clone(),
		tracked: vec!["x.txt".into()],
		untracked: Vec::new(),
		return_focus: Some(egui::Id::unique("discard-origin")),
		focus_cancel: true,
	});
	(state, path)
}

fn button_at(shapes: &[ClippedShape], label: &str) -> egui::Pos2 {
	shapes
		.iter()
		.rev()
		.find_map(|shape| match &shape.shape {
			Shape::Text(text) if text.galley.text() == label => {
				Some(text.visual_bounding_rect().center())
			}
			_ => None,
		})
		.unwrap_or_else(|| panic!("painted button {label} missing"))
}

fn click(ctx: &egui::Context, state: &mut AppState, at: egui::Pos2) {
	frame(
		ctx,
		state,
		vec![
			Event::PointerMoved(at),
			Event::PointerButton {
				pos: at,
				button: PointerButton::Primary,
				pressed: true,
				modifiers: egui::Modifiers::NONE,
			},
		],
	);
	frame(
		ctx,
		state,
		vec![Event::PointerButton {
			pos: at,
			button: PointerButton::Primary,
			pressed: false,
			modifiers: egui::Modifiers::NONE,
		}],
	);
}

#[test]
fn discard_dialog_lists_paths_and_queues_only_confirmed_write() {
	let (mut state, path) = prepared("discard-confirm-ui");
	let ctx = egui::Context::default();
	frame(&ctx, &mut state, Vec::new());
	let shapes = frame(&ctx, &mut state, Vec::new());
	let painted = texts(&shapes);
	for label in ["Discard changes?", "restore  x.txt", "Discard", "Cancel"] {
		assert!(
			painted.iter().any(|text| text == label),
			"missing {label}: {painted:?}"
		);
	}
	assert!(state.writes.running.is_empty() && state.writes.waiting.is_empty());
	click(&ctx, &mut state, button_at(&shapes, "Discard"));
	let request = state.writes.running.get(&path);
	assert!(
		matches!(request.map(|r| &r.kind), Some(WriteKind::Discard { tracked, untracked })
		if tracked == &["x.txt"] && untracked.is_empty())
	);
	assert!(state.pending_discard.is_none());
}

#[test]
fn cancel_discards_nothing_and_restores_focus() {
	let (mut state, _) = prepared("discard-cancel-ui");
	let ctx = egui::Context::default();
	frame(&ctx, &mut state, Vec::new());
	let shapes = frame(&ctx, &mut state, Vec::new());
	assert!(
		ctx.memory(|memory| memory.focused()).is_some(),
		"dialog takes focus"
	);
	click(&ctx, &mut state, button_at(&shapes, "Cancel"));
	assert!(state.pending_discard.is_none());
	assert!(state.writes.running.is_empty() && state.writes.waiting.is_empty());
	assert!(ctx.memory(|memory| memory.has_focus(egui::Id::unique("discard-origin"))));
}

#[test]
fn an_unselected_rows_actions_stay_drawn_under_the_pointer_and_discard_opens_the_dialog() {
	let (mut state, _) = prepared("discard-hover-ui");
	state.pending_discard = None;
	let ctx = egui::Context::default();
	frame(&ctx, &mut state, Vec::new());
	let shapes = frame(&ctx, &mut state, Vec::new());
	let row = button_at(&shapes, "x.txt");
	frame(&ctx, &mut state, vec![Event::PointerMoved(row)]);
	let shapes = frame(&ctx, &mut state, vec![Event::PointerMoved(row)]);
	let discard = button_at(&shapes, "Discard");
	// The flicker seen on screen at 2d2352e: every other frame dropped the actions.
	for n in 0..4 {
		let shapes = frame(&ctx, &mut state, vec![Event::PointerMoved(discard)]);
		assert!(
			texts(&shapes).iter().any(|t| t == "Discard"),
			"Discard drawn on frame {n} with the pointer on it"
		);
	}
	assert!(state.selected_file.is_none(), "the row is still unselected");
	click(&ctx, &mut state, discard);
	assert!(
		state.pending_discard.is_some(),
		"the click reached Discard, not the row"
	);
}

#[test]
fn a_row_discard_click_keeps_that_button_as_the_return_target() {
	let (mut state, _) = prepared("discard-row-focus-ui");
	state.pending_discard = None;
	// A selected row keeps its actions drawn while the pointer sits on them.
	state.selected_file = Some("x.txt".into());
	let ctx = egui::Context::default();
	frame(&ctx, &mut state, Vec::new());
	frame(&ctx, &mut state, Vec::new());
	// Another control holds keyboard focus when the pointer clicks the row's Discard.
	frame(&ctx, &mut state, tab());
	let prior = ctx
		.memory(|memory| memory.focused())
		.unwrap_or_else(|| panic!("Tab focuses a control"));
	let shapes = frame(&ctx, &mut state, Vec::new());
	let discard = button_at(&shapes, "Discard");
	frame(&ctx, &mut state, vec![Event::PointerMoved(discard)]);
	assert!(
		ctx.memory(|memory| memory.has_focus(prior)),
		"the other control still holds focus before the click"
	);
	click(&ctx, &mut state, discard);
	let target = state
		.pending_discard
		.as_ref()
		.and_then(|pending| pending.return_focus)
		.unwrap_or_else(|| panic!("the row click opens the dialog with a return target"));
	assert_ne!(
		target, prior,
		"the return target is not the control that held focus"
	);
	let rect = ctx
		.read_response(target)
		.unwrap_or_else(|| panic!("the return target is a drawn widget"))
		.rect;
	assert!(
		rect.contains(discard),
		"the return target is the clicked Discard button"
	);
}

#[test]
fn keyboard_enter_activates_focused_cancel() {
	let (mut state, _) = prepared("discard-keyboard-ui");
	let ctx = egui::Context::default();
	frame(&ctx, &mut state, Vec::new());
	frame(&ctx, &mut state, Vec::new());
	frame(
		&ctx,
		&mut state,
		vec![Event::Key {
			key: egui::Key::Enter,
			physical_key: None,
			pressed: true,
			repeat: false,
			modifiers: egui::Modifiers::NONE,
		}],
	);
	assert!(state.pending_discard.is_none(), "Enter activates Cancel");
	assert!(state.writes.running.is_empty() && state.writes.waiting.is_empty());
}
