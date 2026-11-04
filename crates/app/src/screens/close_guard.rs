//! Closing the window with an unsaved commit draft asks first. Drafts live only in memory, so a
//! close would lose them (audit-web-design-guidelines-1793709090): the close is cancelled while
//! any draft has text, and a modal names the repositories with Keep editing (the default, also
//! Escape and a click outside) or Close without committing.

use eframe::egui::{self, RichText, ViewportCommand};

use crate::state::AppState;
use crate::theme::*;

pub fn show(ctx: &egui::Context, state: &mut AppState) {
	// Keep editing takes focus on the frame the question opens, away from the draft behind it.
	let mut fresh = false;
	if ctx.input(|i| i.viewport().close_requested()) && !state.close_confirmed {
		let mut drafts: Vec<String> = state
			.commit_messages
			.iter()
			.filter(|(_, text)| !text.trim().is_empty())
			.map(|(path, _)| path.display().to_string())
			.collect();
		if !drafts.is_empty() {
			drafts.sort();
			ctx.send_viewport_cmd(ViewportCommand::CancelClose);
			fresh = state.close_prompt.is_none();
			state.close_prompt = Some(drafts);
		}
	}
	let Some(drafts) = state.close_prompt.clone() else {
		return;
	};
	let mut close: Option<bool> = None;
	let modal = egui::Modal::new(egui::Id::unique("close-with-draft")).show(ctx, |ui| {
		ui.set_max_width(560.0);
		ui.label(
			RichText::new("Close without committing?")
				.size(TEXT_TITLE)
				.color(TEXT_BRIGHT)
				.strong(),
		);
		ui.add_space(6.0);
		let lead = if drafts.len() == 1 {
			"This repository has"
		} else {
			"These repositories have"
		};
		ui.label(
			RichText::new(format!(
				"{lead} an unsaved commit message, which closing discards:"
			))
			.color(TEXT),
		);
		for d in &drafts {
			ui.label(
				RichText::new(d)
					.family(egui::FontFamily::Monospace)
					.size(TEXT_SMALL),
			);
		}
		ui.add_space(10.0);
		ui.horizontal(|ui| {
			let keep = ui.button("Keep editing");
			if fresh {
				keep.request_focus();
			}
			if keep.clicked() {
				close = Some(false);
			}
			let discard =
				egui::Button::new(RichText::new("Close without committing").color(BG_APP))
					.fill(DANGER);
			if ui.add(discard).clicked() {
				close = Some(true);
			}
		});
	});
	if close.is_none() && modal.should_close() {
		close = Some(false);
	}
	match close {
		Some(true) => {
			state.close_prompt = None;
			state.close_confirmed = true;
			ctx.send_viewport_cmd(ViewportCommand::Close);
		}
		Some(false) => state.close_prompt = None,
		None => {}
	}
}
