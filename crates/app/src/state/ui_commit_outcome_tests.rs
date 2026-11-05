//! The commit box's last-write outcome (screens/commit_box.rs show_outcome), drawn through the
//! real frame: each failure heading with git's message and a leftover lock, Committed. with its
//! notice, and a fetch's outcome left to the repository header.

use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;

use super::ui_harness::{frame, repo, texts};
use super::{AppState, WriteKind, WriteOutcome};

fn shown_after(
	kind: WriteKind,
	error: Option<&str>,
	set: impl FnOnce(&mut WriteOutcome),
) -> Vec<String> {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let row = repo("commit-outcome", SystemTime::now());
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	let mut last = WriteOutcome {
		repo: path.clone(),
		kind,
		error: error.map(String::from),
		leftover_lock: None,
		notice: None,
		cancelled: false,
		unknown: false,
	};
	set(&mut last);
	state.writes.last.insert(path, last);
	texts(&frame(&ctx, &mut state, Vec::new()))
}

fn commit() -> WriteKind {
	WriteKind::Commit("Fix it".into())
}

#[test]
fn each_failure_names_itself_and_shows_gits_message() {
	let failed = shown_after(commit(), Some("hook said no"), |_| {});
	assert!(failed.iter().any(|t| t == "Commit failed"), "{failed:?}");
	assert!(failed.iter().any(|t| t == "hook said no"), "{failed:?}");
	let cancelled = shown_after(commit(), Some("cancelled"), |l| l.cancelled = true);
	assert!(
		cancelled.iter().any(|t| t == "Commit cancelled"),
		"{cancelled:?}"
	);
	let unknown = shown_after(commit(), Some("?"), |l| {
		l.cancelled = true;
		l.unknown = true;
	});
	assert!(
		unknown.iter().any(|t| t == "Commit outcome unknown"),
		"{unknown:?}"
	);
	let locked = shown_after(commit(), Some("index.lock exists"), |l| {
		l.leftover_lock = Some(PathBuf::from("C:\\r\\.git\\index.lock"));
	});
	assert!(
		locked
			.iter()
			.any(|t| t.starts_with("A lock file remains at C:\\r\\.git\\index.lock.")),
		"{locked:?}"
	);
}

#[test]
fn a_commit_that_landed_says_so_with_its_notice() {
	let shown = shown_after(commit(), None, |l| {
		l.notice = Some("A later hook was stopped".into())
	});
	assert!(shown.iter().any(|t| t == "Committed."), "{shown:?}");
	assert!(
		shown.iter().any(|t| t == "A later hook was stopped"),
		"{shown:?}"
	);
}

#[test]
fn a_fetch_outcome_is_not_drawn_under_the_commit_box() {
	let shown = shown_after(WriteKind::Fetch, Some("network down"), |_| {});
	assert!(!shown.iter().any(|t| t == "Change failed"), "{shown:?}");
	assert!(!shown.iter().any(|t| t == "Committed."), "{shown:?}");
	// Control: the same outcome as a commit is drawn, so the frame does reach the box.
	let commit_failed = shown_after(commit(), Some("network down"), |_| {});
	assert!(
		commit_failed.iter().any(|t| t == "Commit failed"),
		"{commit_failed:?}"
	);
}
