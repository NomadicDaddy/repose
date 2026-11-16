//! The notification-area icon (close-to-tray): shown only while "Close to tray" is on, with a
//! menu of Open repose and Quit, and a click on the icon reopening the window. Its events arrive
//! on the tray's own window, so the handler forwards them on a channel and wakes a frame; eframe
//! keeps running frames while the window is hidden, so a hidden repose still drains them (and
//! keeps refreshing, clause 8).

use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui::{self, ViewportCommand};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::state::AppState;

const OPEN_ID: &str = "repose-open";
const QUIT_ID: &str = "repose-quit";

/// What a tray event asks of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
	Open,
	Quit,
}

/// The window's side of the tray, kept on AppState so the close guard can read it.
#[derive(Debug, Clone, Copy, Default)]
pub struct TrayState {
	/// The icon is in the notification area. Hiding, and starting hidden, depend on this, not on
	/// the setting alone: with no icon there would be no way back (Morticia Low on 0a3048d).
	pub shown: bool,
	/// The window is hidden to the icon.
	pub hidden: bool,
	/// Quit from the icon's menu is under way, through the close guard.
	pub quitting: bool,
}

/// The tray icon while it is shown, and the channel its events arrive on.
#[derive(Default)]
pub struct Tray {
	icon: Option<TrayIcon>,
	rx: Option<Receiver<TrayAction>>,
	/// The icon could not be built; not tried again until the setting is turned off and on.
	failed: bool,
}

impl Tray {
	/// Shows the icon while the setting is on and removes it when it is turned off, then applies
	/// whatever the icon or its menu asked for since the last frame. If the icon cannot be shown,
	/// a hidden window (a start minimised to the tray) is shown again, since nothing could reopen
	/// it, and closes stay ordinary closes.
	pub fn sync(&mut self, ctx: &egui::Context, state: &mut AppState) {
		if state.config.close_to_tray && self.icon.is_none() && !self.failed {
			let (tx, rx) = channel();
			forward_events(tx, ctx.clone());
			match build_icon() {
				Ok(icon) => {
					self.icon = Some(icon);
					self.rx = Some(rx);
				}
				Err(e) => {
					self.failed = true;
					state.notice = Some(format!("Could not show the tray icon: {e}"));
				}
			}
		} else if !state.config.close_to_tray {
			// Dropping the icon removes it from the notification area (clause 1).
			self.icon = None;
			self.rx = None;
			self.failed = false;
		}
		state.tray.shown = self.icon.is_some();
		if !state.tray.shown && state.tray.hidden {
			show_window(ctx, state);
		}
		let actions: Vec<TrayAction> = self
			.rx
			.as_ref()
			.map(|rx| rx.try_iter().collect())
			.unwrap_or_default();
		for action in actions {
			apply(ctx, state, action);
		}
	}
}

/// Open shows the window where it was and gives it focus; Quit closes it through the close guard,
/// which asks first when a commit draft has text (clause 4).
pub fn apply(ctx: &egui::Context, state: &mut AppState, action: TrayAction) {
	match action {
		TrayAction::Open => show_window(ctx, state),
		TrayAction::Quit => {
			state.tray.quitting = true;
			ctx.send_viewport_cmd(ViewportCommand::Close);
		}
	}
	ctx.request_repaint();
}

/// Shows a hidden window and brings it to the front.
pub fn show_window(ctx: &egui::Context, state: &mut AppState) {
	state.tray.hidden = false;
	ctx.send_viewport_cmd(ViewportCommand::Visible(true));
	ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
	ctx.send_viewport_cmd(ViewportCommand::Focus);
}

/// The icon with its tooltip and its two-item menu. The menu's items carry their own text, which
/// is what a screen reader reads (clause 3).
fn build_icon() -> Result<TrayIcon, String> {
	let menu = Menu::new();
	let open = MenuItem::with_id(OPEN_ID, "Open repose", true, None);
	let quit = MenuItem::with_id(QUIT_ID, "Quit", true, None);
	menu.append_items(&[&open, &quit])
		.map_err(|e| e.to_string())?;
	TrayIconBuilder::new()
		.with_tooltip("repose")
		.with_icon(icon_image()?)
		.with_menu(Box::new(menu))
		.with_menu_on_left_click(false)
		.build()
		.map_err(|e| e.to_string())
}

/// The program icon (program-icon), decoded from the same PNG the window uses.
fn icon_image() -> Result<tray_icon::Icon, String> {
	let data = eframe::icon_data::from_png_bytes(crate::ICON_PNG).map_err(|e| e.to_string())?;
	tray_icon::Icon::from_rgba(data.rgba, data.width, data.height).map_err(|e| e.to_string())
}

/// Turns the icon's and the menu's events into actions on `tx` and wakes a frame to apply them.
/// A left click (on release) or a double click opens; the menu's items open or quit.
fn forward_events(tx: Sender<TrayAction>, ctx: egui::Context) {
	let icon_tx = tx.clone();
	let icon_ctx = ctx.clone();
	TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
		if let Some(action) = icon_action(&event) {
			let _ = icon_tx.send(action);
			icon_ctx.request_repaint();
		}
	}));
	MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
		if let Some(action) = menu_action(event.id.as_ref()) {
			let _ = tx.send(action);
			ctx.request_repaint();
		}
	}));
}

/// The action an icon event asks for: Open on a left click's release or a double click.
pub fn icon_action(event: &TrayIconEvent) -> Option<TrayAction> {
	match event {
		TrayIconEvent::Click {
			button: MouseButton::Left,
			button_state: MouseButtonState::Up,
			..
		}
		| TrayIconEvent::DoubleClick {
			button: MouseButton::Left,
			..
		} => Some(TrayAction::Open),
		_ => None,
	}
}

/// The action a menu item asks for, by its id.
pub fn menu_action(id: &str) -> Option<TrayAction> {
	match id {
		OPEN_ID => Some(TrayAction::Open),
		QUIT_ID => Some(TrayAction::Quit),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use tray_icon::dpi::PhysicalPosition;
	use tray_icon::{Rect, TrayIconId};

	use super::*;

	fn click(button: MouseButton, button_state: MouseButtonState) -> TrayIconEvent {
		TrayIconEvent::Click {
			id: TrayIconId::new("t"),
			position: PhysicalPosition::default(),
			rect: Rect::default(),
			button,
			button_state,
		}
	}

	/// close-to-tray clause 3: a left click opens on release, a right click is left to the menu,
	/// and the menu's two items open and quit.
	#[test]
	fn icon_and_menu_events_map_to_open_and_quit() {
		use MouseButtonState::{Down, Up};
		assert_eq!(
			icon_action(&click(MouseButton::Left, Up)),
			Some(TrayAction::Open)
		);
		assert_eq!(icon_action(&click(MouseButton::Left, Down)), None);
		assert_eq!(icon_action(&click(MouseButton::Right, Up)), None);
		assert_eq!(menu_action(OPEN_ID), Some(TrayAction::Open));
		assert_eq!(menu_action(QUIT_ID), Some(TrayAction::Quit));
		assert_eq!(menu_action("something-else"), None);
	}
}
