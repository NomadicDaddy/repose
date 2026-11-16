//! The first close to tray says where repose went (close-to-tray clause 9; Phillip #54735, and
//! #54765 on #54762: inside repose's own window, not as a Windows notification). The window stays
//! up with a short notice and an OK; OK records the notice as shown, in config.toml's
//! `tray_notice_shown` alone so it never repeats across restarts, and hides the window. Every later
//! close hides at once.

use eframe::egui::{self, RichText, ViewportCommand};
use repose_core::settings;

use crate::state::AppState;
use crate::theme::*;

/// The text the notice shows.
pub const NOTICE: &str =
	"repose keeps running in the notification area. Click its icon to reopen it.";

fn open_id() -> egui::Id {
	egui::Id::unique("tray-notice-open")
}

/// A close with "Close to tray" on: the notice the first time, otherwise straight to the tray.
pub fn close_to_tray(ctx: &egui::Context, state: &mut AppState) {
	ctx.send_viewport_cmd(ViewportCommand::CancelClose);
	if state.config.tray_notice_shown {
		hide(ctx, state);
	} else {
		ctx.data_mut(|d| d.insert_temp(open_id(), true));
	}
}

/// Hides the window to the notification area.
pub fn hide(ctx: &egui::Context, state: &mut AppState) {
	ctx.send_viewport_cmd(ViewportCommand::Visible(false));
	state.tray.hidden = true;
}

/// The notice, while it is open. OK (also Enter, Escape or a click outside) records it and hides.
pub fn show(ctx: &egui::Context, state: &mut AppState) {
	if !ctx.data(|d| d.get_temp::<bool>(open_id())).unwrap_or(false) {
		return;
	}
	let mut done = false;
	let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(16.0);
	let modal = egui::Modal::new(egui::Id::unique("tray-notice"))
		.frame(frame)
		.show(ctx, |ui| {
			ui.set_max_width(480.0);
			ui.label(
				RichText::new("Closed to the notification area")
					.size(TEXT_TITLE)
					.color(TEXT_BRIGHT)
					.strong(),
			);
			ui.add_space(6.0);
			ui.label(RichText::new(NOTICE).color(TEXT));
			ui.add_space(10.0);
			let ok = ui.button("OK");
			ok.request_focus();
			if ok.clicked() {
				done = true;
			}
		});
	if done || modal.should_close() {
		ctx.data_mut(|d| d.remove::<bool>(open_id()));
		state.config.tray_notice_shown = true;
		if let Some(path) = &state.config_path
			&& let Err(e) = settings::save_tray_notice_shown(path)
		{
			state.notice = Some(e.to_string());
		}
		hide(ctx, state);
	}
}
