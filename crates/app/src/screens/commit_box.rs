//! The commit box: a message, Commit on Ctrl+Enter or the button, and the last write's outcome.

use eframe::egui::{self, Key, RichText, Ui};
use repose_core::model::BranchState;

use crate::state::{AppState, WriteKind};
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
	let busy = state.writes.busy_for(&repo.path);
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
	let can_commit = staged > 0 && busy.is_none() && !message.trim().is_empty();
	ui.horizontal(|ui| {
		let button = ui.add_enabled(can_commit, egui::Button::new("Commit"));
		// A commit waits for its hooks however long they take; Cancel kills git and the hook's
		// whole process tree, and the message stays in the box (stage-and-commit clauses 8, 9).
		let committing = state
			.writes
			.running
			.get(&repo.path)
			.is_some_and(|r| matches!(r.kind, WriteKind::Commit(_)));
		if committing
			&& ui
				.button("Cancel")
				.on_hover_text("Stop git and its hooks; the index is left as git left it")
				.clicked()
		{
			state.writes.cancel(&repo.path);
		}
		let reason = if let Some(label) = busy {
			format!("{label}…")
		} else if staged == 0 {
			"Nothing staged".to_string()
		} else if message.trim().is_empty() {
			"Write a message".to_string()
		} else {
			String::new()
		};
		if !reason.is_empty() {
			ui.label(RichText::new(reason).size(TEXT_SMALL).color(MUTED));
		}
		if (button.clicked() || submit) && can_commit {
			let ctx = ui.ctx().clone();
			state.request_write(repo.path.clone(), WriteKind::Commit(message), &ctx);
		}
	});
	if let Some(last) = state.writes.last.get(&repo.path) {
		match &last.error {
			Some(error) => {
				let what = match &last.kind {
					WriteKind::Commit(_) if last.unknown => "Commit outcome unknown",
					WriteKind::Commit(_) if last.cancelled => "Commit cancelled",
					WriteKind::Commit(_) => "Commit failed",
					WriteKind::Discard { .. } => "Discard failed",
					_ => "Change failed",
				};
				ui.label(RichText::new(what).color(DANGER).strong());
				// A hook's output is kept whole (up to 40 lines) and scrolls in place.
				egui::ScrollArea::vertical()
					.id_salt(("write-error", &repo.path))
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
			None => {
				if let WriteKind::Commit(_) = last.kind {
					ui.label(RichText::new("Committed.").size(TEXT_SMALL).color(ACCENT));
					if let Some(n) = &last.notice {
						ui.label(RichText::new(n.as_str()).size(TEXT_SMALL).color(WARN));
					}
				}
			}
		}
	}
}
