//! The discard confirmation: every affected path, listed, before anything runs.

use eframe::egui::{self, RichText};

use crate::state::{AppState, WriteKind};
use crate::theme::*;

pub fn show(ctx: &egui::Context, state: &mut AppState) {
	let Some(pending) = state.pending_discard.clone() else {
		return;
	};
	let total = pending.tracked.len() + pending.untracked.len();
	let mut decision: Option<bool> = None;
	// A modal, so nothing behind it takes a click or a key while the question stands; Escape or
	// a click on the backdrop is Cancel (stage-and-commit: the confirm window was not modal).
	let modal = egui::Modal::new(egui::Id::unique("discard-confirm")).show(ctx, |ui| {
		ui.set_max_width(640.0);
		ui.label(
			RichText::new("Discard changes?")
				.size(TEXT_TITLE)
				.color(TEXT_BRIGHT)
				.strong(),
		);
		ui.add_space(6.0);
		ui.label(
			RichText::new(format!(
				"This cannot be undone. {} in {}:",
				super::count_label(total, "path", "paths"),
				pending.repo.display()
			))
			.color(TEXT),
		);
		ui.add_space(6.0);
		egui::ScrollArea::vertical()
			.max_height(260.0)
			.show(ui, |ui| {
				for p in &pending.tracked {
					path_row(ui, "restore", p, None);
				}
				for p in &pending.untracked {
					path_row(ui, "remove", p, Some(DANGER));
				}
			});
		ui.add_space(10.0);
		ui.horizontal(|ui| {
			let cancel = ui.push_id("discard-cancel", |ui| ui.button("Cancel")).inner;
			if pending.focus_cancel {
				cancel.request_focus();
			}
			if cancel.clicked() {
				decision = Some(false);
			}
			let discard = egui::Button::new(RichText::new("Discard").color(BG_APP)).fill(DANGER);
			if ui.add(discard).clicked() {
				decision = Some(true);
			}
		});
	});
	if let Some(current) = state.pending_discard.as_mut() {
		current.focus_cancel = false;
	}
	if decision.is_none() && modal.should_close() {
		decision = Some(false);
	}
	if decision.is_some()
		&& state
			.selected_repo()
			.is_some_and(|repo| repo.path == pending.repo)
		&& let Some(id) = pending.return_focus
	{
		ctx.memory_mut(|memory| memory.request_focus(id));
	}
	match decision {
		Some(true) => {
			state.pending_discard = None;
			state.request_write(
				pending.repo,
				WriteKind::Discard {
					tracked: pending.tracked,
					untracked: pending.untracked,
				},
				ctx,
			);
		}
		Some(false) => state.pending_discard = None,
		None => {}
	}
}

/// One affected path, its verb padded so the paths line up, painted as the diff lines are: its
/// one node in the accessibility tree reads "restore x.txt" or "remove x.txt" as read-only text
/// that adds no Tab stop, so a screen reader reaches every path the user is asked to confirm
/// (audit-frontend-1793999566). A label widget would add a second node with the padded text.
fn path_row(ui: &mut egui::Ui, verb: &str, path: &str, colour: Option<egui::Color32>) {
	let colour = colour.unwrap_or_else(|| ui.visuals().text_color());
	let galley = egui::WidgetText::from(
		RichText::new(format!("{verb:<9}{path}"))
			.family(egui::FontFamily::Monospace)
			.size(TEXT_SMALL)
			.color(colour),
	)
	.into_galley(
		ui,
		Some(egui::TextWrapMode::Wrap),
		ui.available_width(),
		egui::TextStyle::Body,
	);
	let (rect, row) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
	super::a11y::text(
		ui,
		row.id,
		rect,
		egui::Role::Label,
		&format!("{verb} {path}"),
	);
	ui.painter().galley(rect.min, galley, colour);
}
