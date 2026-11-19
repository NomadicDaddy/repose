//! The commit box's Suggest message row (ai-commit-message-suggestions clauses 3, 4, 6 and 7):
//! shown only while Settings has a provider picked, after Commit so the box's own tab order is
//! unchanged. A busy line and Cancel while it runs, the replace question when the user's own
//! message is in the box, and the reason a suggestion failed or the files it was written without.

use std::path::Path;

use eframe::egui::{self, RichText, Ui};

use super::a11y;
use crate::state::{AppState, SuggestNote};
use crate::theme::*;

/// What pressing the button does, on its hover.
const HELP: &str = "Sends the staged changes to Claude Code and fills the message with its \
suggestion. Nothing is committed until you press Commit.";

pub fn show(ui: &mut Ui, state: &mut AppState, repo: &Path, staged: usize) {
	if !state.suggestions_on() {
		return;
	}
	let ctx = ui.ctx().clone();
	ui.horizontal(|ui| {
		if state.suggest.is_running(repo) {
			ui.add(egui::Spinner::new());
			let busy = "Suggesting a message…";
			let shown = ui.label(RichText::new(busy).size(TEXT_SMALL).color(MUTED));
			a11y::live(&shown, busy);
			if ui
				.button("Cancel suggestion")
				.on_hover_text("Stop Claude Code; the message stays as it is")
				.clicked()
			{
				state.cancel_suggestion(repo);
			}
		} else if ui
			.add_enabled(staged > 0, egui::Button::new("Suggest message"))
			.on_hover_text(HELP)
			.on_disabled_hover_text("Stage the changes to describe first")
			.clicked()
		{
			state.suggest_message(repo, &ctx);
		}
	});
	if state.suggest.pending.contains_key(repo) {
		ui.horizontal_wrapped(|ui| {
			let question = "A suggestion is ready. Replace your message with it?";
			let shown = ui.label(RichText::new(question).size(TEXT_SMALL).color(TEXT));
			a11y::live(&shown, question);
			if ui.button("Replace").clicked() {
				state.answer_suggestion(repo, true);
			}
			if ui.button("Keep mine").clicked() {
				state.answer_suggestion(repo, false);
			}
		});
	}
	match state.suggest.notes.get(repo) {
		Some(SuggestNote::Failed(why)) => {
			let shown = ui.label(RichText::new(why).size(TEXT_SMALL).color(DANGER));
			a11y::live(&shown, why);
		}
		Some(SuggestNote::LeftOut(files)) => {
			let note = format!(
				"Suggested without the diffs of {} large staged file{}: {}",
				files.len(),
				if files.len() == 1 { "" } else { "s" },
				files.join(", ")
			);
			ui.label(RichText::new(note).size(TEXT_SMALL).color(WARN));
		}
		None => {}
	}
}
