//! remote-pull-push clauses 3-5 through the real panes: Pull and Push beside Fetch in the History
//! header and the row menu, each queued only on its click and one remote action at a time; the
//! row and header say which one runs, with Cancel beside it; Pull waits for a clean tracked tree
//! and both need an upstream; a failure shows under "Pull failed" or "Push failed" with git's
//! message and not in the commit box. The harness's repository folder does not exist, so a click
//! fails at spawn and never reaches a network.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use eframe::egui::{self, Key};
use repose_core::config::Config;
use repose_core::git::remote::RemoteEntry;

use super::ui_harness::{click, frame, key, repo, text_at, texts};
use super::{AppState, View, WriteKind, WriteOutcome};

/// A row with an origin; `upstream` sets the branch's upstream, `tracked` its unstaged count.
fn with_branch(name: &str, upstream: bool, tracked: usize) -> (AppState, PathBuf) {
	let mut state = AppState::new(Config::default());
	let mut row = repo(name, SystemTime::now());
	if let Some(status) = row.status.as_mut() {
		status.unstaged = tracked;
		status.untracked = 2;
		status.upstream = upstream.then(|| "origin/main".to_string());
		status.ahead = upstream.then_some(1);
		status.behind = upstream.then_some(1);
	}
	let path = row.path.clone();
	state.requested_selection = Some(path.clone());
	assert!(state.merge_discovered(vec![row]), "the row is selected");
	let origin = RemoteEntry {
		name: "origin".into(),
		url: "https://example.invalid/team/thing.git".into(),
		push_url: None,
	};
	state.remotes.insert(path.clone(), Ok(vec![origin]));
	(state, path)
}

fn has(shapes: &[egui::epaint::ClippedShape], fragment: &str) -> bool {
	texts(shapes).iter().any(|t| t.contains(fragment))
}

/// Clicks the History header's `label` button and returns what is queued or running after it.
fn click_header(ctx: &egui::Context, state: &mut AppState, label: &str) -> Option<WriteKind> {
	state.view = View::History;
	let shown = frame(ctx, state, Vec::new());
	let at = text_at(&shown, label).unwrap_or_else(|| panic!("no {label}: {:?}", texts(&shown)));
	frame(ctx, state, click(at));
	let path = state.selected.clone()?;
	state.writes.remote_write(&path).cloned()
}

#[test]
fn the_history_header_pull_queues_one_pull_and_the_row_and_header_show_it() {
	let ctx = egui::Context::default();
	let (mut state, path) = with_branch("pull-header", true, 0);
	assert_eq!(
		click_header(&ctx, &mut state, "Pull"),
		Some(WriteKind::Pull)
	);
	let busy = frame(&ctx, &mut state, Vec::new());
	assert!(has(&busy, "pulling…"), "the row pill: {:?}", texts(&busy));
	assert!(
		has(&busy, "Pulling…"),
		"the header note: {:?}",
		texts(&busy)
	);
	assert!(has(&busy, "Cancel"), "Cancel sits beside the running pull");
	// A second Pull, or a Push, while it runs does nothing: one remote action at a time.
	for label in ["Pull", "Push", "Fetch"] {
		let at = text_at(&busy, label).unwrap_or_else(|| panic!("no {label}"));
		frame(&ctx, &mut state, click(at));
	}
	let queued = state
		.writes
		.waiting
		.iter()
		.filter(|r| r.repo == path)
		.count();
	assert_eq!(queued, 0, "nothing is queued behind the running pull");
}

#[test]
fn uncommitted_tracked_changes_hold_pull_but_not_push() {
	let ctx = egui::Context::default();
	let (mut state, _path) = with_branch("pull-dirty", true, 3);
	assert_eq!(
		click_header(&ctx, &mut state, "Pull"),
		None,
		"a disabled Pull queues nothing"
	);
	assert_eq!(
		click_header(&ctx, &mut state, "Push"),
		Some(WriteKind::Push)
	);
}

#[test]
fn untracked_files_alone_do_not_hold_pull() {
	let ctx = egui::Context::default();
	let (mut state, _path) = with_branch("pull-untracked", true, 0);
	assert_eq!(
		click_header(&ctx, &mut state, "Pull"),
		Some(WriteKind::Pull)
	);
}

#[test]
fn with_no_upstream_pull_and_push_queue_nothing_and_fetch_still_runs() {
	let ctx = egui::Context::default();
	let (mut state, _path) = with_branch("no-upstream", false, 0);
	assert_eq!(click_header(&ctx, &mut state, "Pull"), None);
	assert_eq!(click_header(&ctx, &mut state, "Push"), None);
	assert_eq!(
		click_header(&ctx, &mut state, "Fetch"),
		Some(WriteKind::Fetch)
	);
}

#[test]
fn the_row_menu_push_queues_one_push() {
	let ctx = egui::Context::default();
	let (mut state, path) = with_branch("push-menu", true, 0);
	frame(&ctx, &mut state, Vec::new());
	frame(&ctx, &mut state, key(Key::Menu));
	let open = frame(&ctx, &mut state, Vec::new());
	let at = text_at(&open, "Push").unwrap_or_else(|| panic!("no Push item: {:?}", texts(&open)));
	frame(&ctx, &mut state, click(at));
	assert_eq!(state.writes.remote_write(&path), Some(&WriteKind::Push));
	let busy = frame(&ctx, &mut state, Vec::new());
	assert!(has(&busy, "pushing…"), "the row pill: {:?}", texts(&busy));
}

fn failed(path: &Path, kind: WriteKind, message: &str) -> WriteOutcome {
	WriteOutcome {
		repo: path.to_path_buf(),
		kind,
		error: Some(message.into()),
		leftover_lock: None,
		notice: None,
		cancelled: false,
		unknown: false,
	}
}

#[test]
fn a_failed_pull_or_push_shows_gits_message_under_its_own_heading_only() {
	let ctx = egui::Context::default();
	for (kind, heading, message) in [
		(
			WriteKind::Pull,
			"Pull failed",
			"fatal: Not possible to fast-forward, aborting.",
		),
		(
			WriteKind::Push,
			"Push failed",
			"! [rejected]        main -> main (fetch first)",
		),
	] {
		let (mut state, path) = with_branch("remote-failed", true, 0);
		state.finish_write(failed(&path, kind, message), &ctx);
		let shapes = frame(&ctx, &mut state, Vec::new());
		assert!(has(&shapes, heading), "{:?}", texts(&shapes));
		assert!(has(&shapes, message), "git's own message is shown");
		assert!(
			!has(&shapes, "Change failed"),
			"the commit box does not repeat it"
		);
		assert!(
			!has(&shapes, "Fetched "),
			"a failed pull claims no fetch time"
		);
	}
}
