//! The commit box: a message, Commit on Ctrl+Enter or the button, and the last write's outcome.

use std::path::Path;

use eframe::egui::{self, Key, RichText, Ui};
use repose_core::model::BranchState;

use super::a11y;
use crate::state::{AppState, WriteKind, WriteOutcome};
use crate::theme::*;

pub fn show(ui: &mut Ui, state: &mut AppState) {
	let Some(repo) = state.selected_repo().cloned() else {
		return;
	};
	let staged = repo.changes.as_ref().map(|c| c.staged.len()).unwrap_or(0);
	let branch = match &repo.branch {
		BranchState::Branch(b) => b.clone(),
		BranchState::Detached => "detached HEAD".to_string(),
		BranchState::Unborn => "a new branch".to_string(),
	};
	ui.add_space(10.0);
	let label = ui.label("Commit message");
	let hint = format!("Message (Ctrl+Enter to commit on {branch})");
	let draft = state.commit_messages.entry(repo.path.clone()).or_default();
	let edit = egui::TextEdit::multiline(draft)
		.hint_text(hint)
		.desired_rows(3)
		.desired_width(f32::INFINITY);
	let response = ui
		.add(edit)
		.labelled_by(label.id)
		.accessible_name("Commit message");
	if response.has_focus() {
		// Letters typed here are the message's, never the list's type-ahead (Phillip #31623).
		state.text_focus = true;
	}
	let submit =
		response.has_focus() && ui.input(|i| i.modifiers.command && i.key_pressed(Key::Enter));
	let message = draft.clone();
	button_row(ui, state, &repo.path, staged, message, submit);
	// A fetch's outcome is shown under the repository header, where every pane can see it.
	if let Some(last) = state
		.writes
		.last
		.get(&repo.path)
		.filter(|l| l.kind != WriteKind::Fetch)
	{
		show_outcome(ui, &repo.path, last);
	}
}

/// Commit, Cancel while a commit runs, and why Commit is disabled.
fn button_row(
	ui: &mut Ui,
	state: &mut AppState,
	repo: &Path,
	staged: usize,
	message: String,
	submit: bool,
) {
	let busy = state.writes.busy_for(repo);
	let reason = disabled_reason(busy, staged, &message);
	let can_commit = reason.is_none();
	ui.horizontal(|ui| {
		let button = ui.add_enabled(can_commit, egui::Button::new("Commit"));
		// A commit waits for its hooks however long they take; Cancel kills git and the hook's
		// whole process tree, and the message stays in the box (stage-and-commit clauses 8, 9).
		let committing = state
			.writes
			.running
			.get(repo)
			.is_some_and(|r| matches!(r.kind, WriteKind::Commit(_)));
		if committing
			&& ui
				.button("Cancel")
				.on_hover_text("Stop git and its hooks; the index is left as git left it")
				.clicked()
		{
			state.writes.cancel(repo);
		}
		if let Some(reason) = &reason {
			let shown = ui.label(RichText::new(reason).size(TEXT_SMALL).color(MUTED));
			if busy.is_some() {
				a11y::live(&shown, reason);
			}
		}
		if (button.clicked() || submit) && can_commit {
			let ctx = ui.ctx().clone();
			state.request_write(repo.to_path_buf(), WriteKind::Commit(message), &ctx);
		}
	});
}

/// Why Commit is disabled, or None when it can run: a write in progress first, then nothing
/// staged, then an empty message.
fn disabled_reason(busy: Option<&str>, staged: usize, message: &str) -> Option<String> {
	if let Some(label) = busy {
		Some(format!("{label}…"))
	} else if staged == 0 {
		Some("Nothing staged".to_string())
	} else if message.trim().is_empty() {
		Some("Write a message".to_string())
	} else {
		None
	}
}

/// The last write's outcome under the box: a failure with git's message (scrolling) and any
/// leftover lock, or Committed. with its notice.
fn show_outcome(ui: &mut Ui, repo: &Path, last: &WriteOutcome) {
	let Some(error) = &last.error else {
		if let WriteKind::Commit(_) = last.kind {
			show_committed(ui, last.notice.as_deref());
		}
		return;
	};
	let what = match &last.kind {
		WriteKind::Commit(_) if last.unknown => "Commit outcome unknown",
		WriteKind::Commit(_) if last.cancelled => "Commit cancelled",
		WriteKind::Commit(_) => "Commit failed",
		WriteKind::Discard { .. } => "Discard failed",
		_ => "Change failed",
	};
	let shown = ui.label(RichText::new(what).color(DANGER).strong());
	a11y::live(&shown, what);
	// A hook's output is kept whole (up to 40 lines) and scrolls in place.
	egui::ScrollArea::vertical()
		.id_salt(("write-error", repo))
		.max_height(160.0)
		.auto_shrink([false, true])
		.show(ui, |ui| {
			ui.label(
				RichText::new(error)
					.size(TEXT_SMALL)
					.color(TEXT)
					.family(egui::FontFamily::Monospace),
			);
		});
	if let Some(lock) = &last.leftover_lock {
		ui.label(
			RichText::new(format!(
				"A lock file remains at {}. repose does not remove it; check nothing else is using the repository, then delete it yourself.",
				lock.display()
			))
			.size(TEXT_SMALL)
			.color(WARN),
		);
	}
}

/// A commit that went through, with the notice git left (such as a hook's warning), if any.
fn show_committed(ui: &mut Ui, notice: Option<&str>) {
	let shown = ui.label(RichText::new("Committed.").size(TEXT_SMALL).color(ACCENT));
	a11y::live(&shown, "Committed.");
	if let Some(n) = notice {
		ui.label(RichText::new(n).size(TEXT_SMALL).color(WARN));
	}
}

#[cfg(test)]
mod tests {
	use super::disabled_reason;

	#[test]
	fn the_disabled_reason_names_the_first_blocker_or_none() {
		assert_eq!(
			disabled_reason(Some("Committing"), 0, ""),
			Some("Committing…".into())
		);
		assert_eq!(
			disabled_reason(None, 0, "msg"),
			Some("Nothing staged".into())
		);
		assert_eq!(
			disabled_reason(None, 2, "  \n"),
			Some("Write a message".into())
		);
		assert_eq!(disabled_reason(None, 2, "msg"), None);
	}
}
