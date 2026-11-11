//! status-line-names-a-missing-root-once and live refresh through the real state: a missing
//! root is named once as not found, a refused existing root gets a plain note on the painted
//! status line, and an overflow during a pass waits and then runs once.

use std::path::PathBuf;

use repose_core::config::Config;
use repose_core::model::{RepoKind, RepoSummary};
use repose_core::watch::Refresh;
use repose_core::watch::watcher::Unwatched;

use super::unwatched_note;
use crate::state::AppState;
use crate::state::ui_harness::{frame, texts};

fn refused(root: &str) -> Unwatched {
	Unwatched {
		root: PathBuf::from(root),
		reason: "Input watch path is neither a file nor a directory.".to_string(),
	}
}

/// Clause 1: a configured root that does not exist reaches the status line once, as not
/// found, and the watcher's refusal of that same root adds nothing.
#[test]
fn a_missing_root_is_named_once_as_not_found() {
	let ctx = eframe::egui::Context::default();
	let gone = std::env::temp_dir().join("repose-test-root-that-does-not-exist");
	let config = Config {
		roots: vec![gone.clone()],
		..Config::default()
	};
	let mut state = AppState::new(config);
	assert_eq!(state.missing_roots, vec![gone.clone()]);
	state.sync_watch(&ctx);
	assert_eq!(state.watching.note, None, "no Not watching clause for it");
	state.checked_at = Some(std::time::SystemTime::now());
	let shapes = frame(&ctx, &mut state, Vec::new());
	let line = texts(&shapes)
		.into_iter()
		.find(|t| t.contains("root not found"))
		.unwrap_or_else(|| panic!("no status line naming the root"));
	assert_eq!(
		line.matches(&*gone.display().to_string()).count(),
		1,
		"named once: {line}"
	);
	assert!(line.contains("1 root not found"), "{line}");
	assert!(!line.contains("neither a file"), "{line}");
}

/// Clause 2: a root that exists but was refused is still reported, in plain words, never
/// with the watcher library's error text.
#[test]
fn an_unwatchable_root_gets_a_plain_note() {
	let missing = vec![PathBuf::from("E:/unplugged")];
	assert_eq!(unwatched_note(&[refused("E:/unplugged")], &missing), None);
	let one = unwatched_note(&[refused("D:/locked")], &missing);
	assert_eq!(
		one.as_deref(),
		Some("Not watching D:/locked: Windows refused a watch on it; the 60 s timer refreshes it")
	);
	let two = unwatched_note(
		&[
			refused("D:/locked"),
			refused("E:/unplugged"),
			refused("F:/share"),
		],
		&missing,
	);
	assert_eq!(
		two.as_deref(),
		Some(
			"Not watching D:/locked and 1 more: Windows refused a watch on them; the 60 s \
			 timer refreshes them"
		)
	);
}

/// Clause 3, the refused case on screen: a root that exists but that the watcher refused
/// shows the plain note in the painted status line, not the library's error, and is not
/// called not found. No real watcher refuses an existing folder on demand, so the refusal
/// is handed in where sync_watch hands in the watcher's own list.
#[test]
fn an_unwatchable_root_shows_its_plain_note_in_the_status_line() {
	let ctx = eframe::egui::Context::default();
	let here = std::env::temp_dir();
	let config = Config {
		roots: vec![here.clone()],
		..Config::default()
	};
	let mut state = AppState::new(config);
	assert!(state.missing_roots.is_empty(), "the temp folder exists");
	state.note_unwatched(&[Unwatched {
		root: here.clone(),
		reason: "Access is denied. (os error 5)".to_string(),
	}]);
	state.checked_at = Some(std::time::SystemTime::now());
	let shapes = frame(&ctx, &mut state, Vec::new());
	let line = texts(&shapes)
		.into_iter()
		.find(|t| t.contains("Not watching"))
		.unwrap_or_else(|| panic!("no status line with the watcher note"));
	let want = format!(
		"Not watching {}: Windows refused a watch on it; the 60 s timer refreshes it",
		here.display()
	);
	assert!(line.contains(&want), "{line}");
	assert!(!line.contains("Access is denied"), "{line}");
	assert!(!line.contains("not found"), "{line}");
}

/// Roger 5Q-1 on 7e70f1f: a lost-events report that arrives while a scan or a pass is
/// running must not be dropped; it waits and runs when the pass has finished.
#[test]
fn an_overflow_during_a_pass_waits_and_then_runs_once() {
	let ctx = eframe::egui::Context::default();
	let mut state = AppState::new(Config::default());
	state.repos = vec![RepoSummary::new(
		PathBuf::from("D:/t/a"),
		RepoKind::Repository,
	)];
	state.backstop_running = true;
	state
		.watching
		.tx
		.send(Refresh::All)
		.unwrap_or_else(|e| panic!("{e}"));
	state.pump_watch(&ctx);
	assert!(state.watching.full_pending, "kept while a pass runs");
	assert!(!state.watching.overflow_pass);
	// The pass finishes: the pending re-read starts now, as an overflow pass.
	state.pass_done(None, &ctx);
	assert!(!state.watching.full_pending);
	assert!(state.watching.overflow_pass);
	assert!(state.backstop_running);
	assert!(
		!state.watching.caught_up,
		"not claimed before the pass completes"
	);
	// A second report inside the quiet window stays pending rather than being lost.
	state
		.watching
		.tx
		.send(Refresh::All)
		.unwrap_or_else(|e| panic!("{e}"));
	state.pump_watch(&ctx);
	assert!(state.watching.full_pending);
	// That pass completes: the second report still waits for quiet, so Watcher caught up is
	// not claimed yet (Roger 5Q-2 on 740b7fd); the pass that answers it will claim it.
	state.pass_done(None, &ctx);
	assert!(!state.watching.caught_up);
	assert!(!state.watching.overflow_pass);
	assert!(
		state.watching.full_pending,
		"inside OVERFLOW_QUIET it waits"
	);
}
