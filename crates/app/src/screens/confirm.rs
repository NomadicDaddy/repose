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
				"This cannot be undone. {} {} in {}:",
				total,
				if total == 1 { "path" } else { "paths" },
				pending.repo.display()
			))
			.color(TEXT),
		);
		ui.add_space(6.0);
		egui::ScrollArea::vertical()
			.max_height(260.0)
			.show(ui, |ui| {
				for p in &pending.tracked {
					ui.label(
						RichText::new(format!("restore  {p}"))
							.family(egui::FontFamily::Monospace)
							.size(TEXT_SMALL),
					);
				}
				for p in &pending.untracked {
					ui.label(
						RichText::new(format!("remove   {p}"))
							.family(egui::FontFamily::Monospace)
							.size(TEXT_SMALL)
							.color(DANGER),
					);
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
