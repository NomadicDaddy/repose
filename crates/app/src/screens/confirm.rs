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
	egui::Window::new("Discard changes?")
		.collapsible(false)
		.resizable(false)
		.anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
		.show(ctx, |ui| {
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
				if ui.button("Cancel").clicked() {
					decision = Some(false);
				}
				let discard =
					egui::Button::new(RichText::new("Discard").color(BG_APP)).fill(DANGER);
				if ui.add(discard).clicked() {
					decision = Some(true);
				}
			});
		});
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
