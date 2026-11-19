//! What the remote actions show (remote-sync and remote-pull-push, A1 as revised by Phillip
//! #35530 and #58496): the refs note beside a repository's branch line; Fetch, Pull and Push in
//! the History header with Cancel while one runs; and a failed one under the repository header
//! with git's own message. The actions themselves run in the write queue, one at a time.

use std::path::Path;

use eframe::egui::{self, FontFamily, RichText, Ui};

use super::{a11y, status_line::clock};
use crate::state::{AppState, WriteKind, find};
use crate::theme::*;

/// The remote actions in the order the History header and the row menu show them.
pub(super) const ACTIONS: [WriteKind; 3] = [WriteKind::Fetch, WriteKind::Pull, WriteKind::Push];

/// The action's name as its button, menu item and failure heading say it.
pub(super) fn name(kind: &WriteKind) -> &'static str {
	match kind {
		WriteKind::Pull => "Pull",
		WriteKind::Push => "Push",
		_ => "Fetch",
	}
}

fn hover(kind: &WriteKind) -> &'static str {
	match kind {
		WriteKind::Pull => "Pull the upstream's commits; fast-forward only, so nothing is merged",
		WriteKind::Push => "Push this branch to its upstream; never forced, and no tags",
		_ => "Fetch from the remote; only Fetch, Pull, Push and Check visibility reach the network",
	}
}

/// "Fetching…", "Pulling…" or "Pushing…" while one is queued or running, "Fetched HH:MM" after a
/// fetch or pull succeeded, and otherwise "Saved refs; no fetch": the counts come from the refs
/// on disk.
pub(super) fn refs_note(state: &AppState, repo: &Path) -> String {
	if let Some(kind) = state.writes.remote_write(repo) {
		format!("{}…", kind.label())
	} else if let Some(at) = state.fetched.get(repo) {
		format!("Fetched {}", clock(*at, state.utc_offset))
	} else {
		"Saved refs; no fetch".to_string()
	}
}

/// The list-wide part of the status line: no fetch at all this session, or how many repositories
/// were fetched and when the last one finished. Each repository's own line says when it was.
pub(super) fn list_note(state: &AppState) -> String {
	match state.fetched.values().max() {
		None => "Saved refs; no fetch".to_string(),
		// After a fetch the count already says where the numbers came from, so "Saved refs; " goes:
		// with it the line wrapped in the fixed sidebar (design-polish clause 3).
		Some(last) => format!(
			"{} fetched, last {}",
			state.fetched.len(),
			clock(*last, state.utc_offset)
		),
	}
}

/// Why this remote action cannot run for this repository now, or None when it can: one remote
/// action at a time, a remote to reach, and for Pull and Push an upstream; Pull also waits for a
/// clean tracked tree, so a fast-forward never meets the user's edits (remote-pull-push clause 5).
pub(super) fn blocked(state: &AppState, repo: &Path, kind: &WriteKind) -> Option<String> {
	if let Some(running) = state.writes.remote_write(repo) {
		return Some(format!("{}…", running.label()));
	}
	match state.remotes.get(repo) {
		None => return Some("Reading the remotes…".into()),
		Some(Ok(list)) if list.is_empty() => return Some("This repository has no remote".into()),
		Some(Err(e)) => return Some(format!("The remotes could not be read: {e}")),
		Some(Ok(_)) => {}
	}
	if *kind == WriteKind::Fetch {
		return None;
	}
	let Some(status) = find(&state.repos, repo).and_then(|r| r.status.as_ref()) else {
		return Some("Reading the status…".into());
	};
	if status.upstream.is_none() {
		return Some("This branch has no upstream".into());
	}
	// No saved counts with an upstream set means its remote branch is gone: Push would create it
	// again, and Phillip's Push never creates a remote branch (#58525).
	if status.ahead.is_none() {
		return Some("This branch's upstream is gone from the remote".into());
	}
	// Push is only ever a fast-forward of what the upstream holds (push::push checks it too).
	if *kind == WriteKind::Push && status.behind.is_some_and(|b| b > 0) {
		return Some("This branch is behind its upstream; Pull first".into());
	}
	if *kind == WriteKind::Pull && status.staged + status.unstaged + status.conflicts > 0 {
		return Some("This branch has uncommitted changes to tracked files".into());
	}
	None
}

/// Fetch, Pull and Push for a pane header, and Cancel while one of them runs: a click queues
/// that action for this repository; Cancel stops git and anything it started.
pub(super) fn buttons(ui: &mut Ui, state: &mut AppState, repo: &Path) {
	for kind in ACTIONS {
		let label = name(&kind);
		match blocked(state, repo, &kind) {
			Some(why) => {
				ui.add_enabled(false, egui::Button::new(label))
					.on_disabled_hover_text(why);
			}
			None => {
				if ui.button(label).on_hover_text(hover(&kind)).clicked() {
					let ctx = ui.ctx().clone();
					state.request_write(repo.to_path_buf(), kind, &ctx);
				}
			}
		}
	}
	let running = state
		.writes
		.running
		.get(repo)
		.is_some_and(|r| r.kind.is_remote());
	if running
		&& ui
			.button("Cancel")
			.on_hover_text("Stop git and anything it started")
			.clicked()
	{
		state.writes.cancel(repo);
	}
}

/// The last remote action's failure for this repository, headed "Pull failed" (or "Pull
/// cancelled", and the same for Fetch and Push) with git's message, which is where a sign-in
/// request, a diverged branch or a rejected push is explained.
pub(super) fn outcome(ui: &mut Ui, state: &AppState, repo: &Path) {
	let Some(last) = state.writes.last.get(repo) else {
		return;
	};
	if !last.kind.is_remote() || state.writes.remote_write(repo).is_some() {
		return;
	}
	let Some(error) = &last.error else {
		return;
	};
	let what = format!(
		"{} {}",
		name(&last.kind),
		if last.cancelled {
			"cancelled"
		} else {
			"failed"
		}
	);
	let shown = ui.label(RichText::new(&what).color(DANGER).strong());
	a11y::live(&shown, &what);
	egui::ScrollArea::vertical()
		.id_salt(("fetch-error", repo))
		.max_height(120.0)
		.auto_shrink([false, true])
		.show(ui, |ui| {
			ui.label(
				RichText::new(error)
					.size(TEXT_SMALL)
					.color(TEXT)
					.family(FontFamily::Monospace),
			);
		});
	// A Cancel during a pull's fast-forward checkout can leave index.lock and a partly updated
	// tree (Morticia L on f61983d); the lock is named as the commit box names one.
	super::commit_box::lock_note(ui, last.leftover_lock.as_deref());
}

#[cfg(test)]
mod tests {
	use repose_core::config::Config;
	use repose_core::git::remote::RemoteEntry;
	use repose_core::model::{RepoKind, RepoSummary, StatusSummary};

	use super::*;

	fn state_with(upstream: bool, unstaged: usize, staged: usize, conflicts: usize) -> AppState {
		let mut state = AppState::new(Config::default());
		let dir = std::env::temp_dir().join("repose-fetch-blocked");
		let mut row = RepoSummary::new(dir, RepoKind::Repository);
		row.status = Some(StatusSummary {
			unstaged,
			staged,
			conflicts,
			untracked: 4,
			upstream: upstream.then(|| "origin/main".to_string()),
			ahead: upstream.then_some(0),
			behind: upstream.then_some(0),
		});
		let path = row.path.clone();
		state.repos = vec![row];
		let origin = RemoteEntry {
			name: "origin".into(),
			url: "https://example.invalid/x.git".into(),
			push_url: None,
		};
		state.remotes.insert(path, Ok(vec![origin]));
		state
	}

	/// The exact tooltips remote-pull-push clause 5 names, for each count that holds Pull.
	#[test]
	fn pull_and_push_say_exactly_why_they_are_off() {
		let dirty = "This branch has uncommitted changes to tracked files";
		for (unstaged, staged, conflicts) in [(1, 0, 0), (0, 1, 0), (0, 0, 1)] {
			let state = state_with(true, unstaged, staged, conflicts);
			let path = state.repos[0].path.clone();
			assert_eq!(
				blocked(&state, &path, &WriteKind::Pull).as_deref(),
				Some(dirty)
			);
			assert_eq!(blocked(&state, &path, &WriteKind::Push), None);
		}
		let clean = state_with(true, 0, 0, 0);
		let path = clean.repos[0].path.clone();
		assert_eq!(
			blocked(&clean, &path, &WriteKind::Pull),
			None,
			"untracked files alone"
		);
		let none = state_with(false, 0, 0, 0);
		for kind in [WriteKind::Pull, WriteKind::Push] {
			assert_eq!(
				blocked(&none, &path, &kind).as_deref(),
				Some("This branch has no upstream")
			);
		}
		assert_eq!(blocked(&none, &path, &WriteKind::Fetch), None);
		// An upstream whose remote branch is gone has no saved counts: Push would recreate it.
		let mut gone = state_with(true, 0, 0, 0);
		if let Some(s) = gone.repos[0].status.as_mut() {
			(s.ahead, s.behind) = (None, None);
		}
		for kind in [WriteKind::Pull, WriteKind::Push] {
			assert_eq!(
				blocked(&gone, &path, &kind).as_deref(),
				Some("This branch's upstream is gone from the remote")
			);
		}
		// Behind its upstream: Pull can catch up, Push would not be a fast-forward.
		let mut behind = state_with(true, 0, 0, 0);
		if let Some(s) = behind.repos[0].status.as_mut() {
			s.behind = Some(2);
		}
		assert_eq!(blocked(&behind, &path, &WriteKind::Pull), None);
		assert_eq!(
			blocked(&behind, &path, &WriteKind::Push).as_deref(),
			Some("This branch is behind its upstream; Pull first")
		);
	}
}
