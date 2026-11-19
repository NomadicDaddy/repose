//! ai-commit-message-suggestions through the real commit box: no control while Settings has it
//! off; with it on, Suggest message is offered and nothing starts until it is pressed; an answer
//! fills an empty message, asks before replacing typed text and never commits; a failure leaves
//! the message as it was and says why; the busy line and Cancel show while it runs. No test
//! presses Suggest message on a row with staged changes, so Claude Code never runs here.

use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui;
use repose_core::config::{Config, SuggestMessages};
use repose_core::git::Cancel;
use repose_core::model::{ChangeKind, FileChange};
use repose_core::suggest::Suggestion;

use super::AppState;
use super::ui_harness::{click, frame, repo, text_at, texts};

/// A selected row with `staged` staged files, and suggestions set to `provider`.
fn with_staged(provider: SuggestMessages, staged: usize) -> (egui::Context, AppState, PathBuf) {
	let mut state = AppState::new(Config {
		suggest_messages: provider,
		..Config::default()
	});
	let mut row = repo("suggest", SystemTime::now());
	if let Some(changes) = row.changes.as_mut() {
		changes.staged = (0..staged)
			.map(|i| FileChange {
				path: format!("s{i}.txt"),
				kind: ChangeKind::Modified,
				from: None,
			})
			.collect();
	}
	if let Some(status) = row.status.as_mut() {
		status.staged = staged;
	}
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	(egui::Context::default(), state, path)
}

/// Where `label` is drawn, or a failure listing what is.
fn at(shapes: &[egui::epaint::ClippedShape], label: &str) -> egui::Pos2 {
	text_at(shapes, label).unwrap_or_else(|| panic!("no {label}: {:?}", texts(shapes)))
}

fn shown(ctx: &egui::Context, state: &mut AppState) -> Vec<String> {
	texts(&frame(ctx, state, Vec::new()))
}

fn answer(message: &str, left_out: &[&str]) -> Result<Suggestion, String> {
	Ok(Suggestion {
		message: message.into(),
		left_out: left_out.iter().map(|s| s.to_string()).collect(),
	})
}

#[test]
fn with_suggestions_off_there_is_no_control_and_nothing_can_start() {
	let (ctx, mut state, path) = with_staged(SuggestMessages::Off, 2);
	let texts = shown(&ctx, &mut state);
	assert!(
		texts.iter().any(|t| t == "Commit"),
		"the commit box shows: {texts:?}"
	);
	assert!(!texts.iter().any(|t| t.contains("Suggest")), "{texts:?}");
	state.suggest_message(&path, &ctx);
	assert!(!state.suggest.is_running(&path), "off runs nothing");
}

#[test]
fn with_suggestions_on_the_control_is_offered_and_nothing_starts_on_its_own() {
	let (ctx, mut state, path) = with_staged(SuggestMessages::Claude, 2);
	for _ in 0..5 {
		let texts = shown(&ctx, &mut state);
		assert!(texts.iter().any(|t| t == "Suggest message"), "{texts:?}");
	}
	state.commit_messages.insert(path.clone(), "typing".into());
	shown(&ctx, &mut state);
	assert!(
		!state.suggest.is_running(&path),
		"frames and typing send nothing"
	);
	assert!(state.suggest.notes.is_empty() && state.suggest.pending.is_empty());
}

#[test]
fn with_nothing_staged_the_control_does_nothing() {
	let (ctx, mut state, path) = with_staged(SuggestMessages::Claude, 0);
	let before = frame(&ctx, &mut state, Vec::new());
	let spot = at(&before, "Suggest message");
	frame(&ctx, &mut state, click(spot));
	shown(&ctx, &mut state);
	assert!(
		!state.suggest.is_running(&path),
		"a disabled control starts nothing"
	);
}

#[test]
fn an_answer_fills_an_empty_message_and_never_commits() {
	let (ctx, mut state, path) = with_staged(SuggestMessages::Claude, 1);
	state.merge_suggestion(path.clone(), answer("Add the row", &[]));
	assert_eq!(
		state.commit_messages.get(&path).map(String::as_str),
		Some("Add the row")
	);
	shown(&ctx, &mut state);
	assert!(
		state.writes.running.is_empty() && state.writes.last.is_empty(),
		"nothing committed"
	);
}

#[test]
fn an_answer_over_typed_text_asks_before_replacing_it() {
	let (ctx, mut state, path) = with_staged(SuggestMessages::Claude, 1);
	state.commit_messages.insert(path.clone(), "mine".into());
	state.merge_suggestion(path.clone(), answer("theirs", &[]));
	assert_eq!(
		state.commit_messages.get(&path).map(String::as_str),
		Some("mine")
	);
	let asked = frame(&ctx, &mut state, Vec::new());
	assert!(
		texts(&asked)
			.iter()
			.any(|t| t.contains("Replace your message with it?"))
	);
	frame(&ctx, &mut state, click(at(&asked, "Keep mine")));
	assert_eq!(
		state.commit_messages.get(&path).map(String::as_str),
		Some("mine")
	);
	assert!(state.suggest.pending.is_empty());
	state.merge_suggestion(path.clone(), answer("theirs", &[]));
	let asked = frame(&ctx, &mut state, Vec::new());
	frame(&ctx, &mut state, click(at(&asked, "Replace")));
	assert_eq!(
		state.commit_messages.get(&path).map(String::as_str),
		Some("theirs")
	);
}

#[test]
fn a_failure_keeps_the_message_and_says_why_and_left_out_files_are_named() {
	let (ctx, mut state, path) = with_staged(SuggestMessages::Claude, 2);
	state.commit_messages.insert(path.clone(), "mine".into());
	state.merge_suggestion(path.clone(), Err("Claude Code: Not logged in".into()));
	assert_eq!(
		state.commit_messages.get(&path).map(String::as_str),
		Some("mine")
	);
	let texts = shown(&ctx, &mut state);
	assert!(
		texts.iter().any(|t| t == "Claude Code: Not logged in"),
		"{texts:?}"
	);
	assert!(texts.iter().any(|t| t == "Commit"), "the box stays usable");
	state.commit_messages.insert(path.clone(), String::new());
	state.merge_suggestion(path.clone(), answer("Add data", &["big.json"]));
	let texts = shown(&ctx, &mut state);
	assert!(
		texts
			.iter()
			.any(|t| t == "Suggested without the diffs of 1 large staged file: big.json"),
		"{texts:?}"
	);
}

#[test]
fn while_it_runs_the_box_shows_it_is_busy_and_cancel_stops_it() {
	let (ctx, mut state, path) = with_staged(SuggestMessages::Claude, 1);
	let cancel = Cancel::new();
	state.suggest.running.insert(path.clone(), cancel.clone());
	let busy = frame(&ctx, &mut state, Vec::new());
	let texts = texts(&busy);
	assert!(
		texts.iter().any(|t| t == "Suggesting a message…"),
		"{texts:?}"
	);
	assert!(
		!texts.iter().any(|t| t == "Suggest message"),
		"no second start while it runs"
	);
	frame(&ctx, &mut state, click(at(&busy, "Cancel suggestion")));
	assert!(cancel.is_cancelled());
}
