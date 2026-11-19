//! remote-visibility: whether origin is Public, Private or Unknown, read through gh on the user's
//! Check visibility only (A1), and kept in memory per remote URL with the time it was read. It is
//! never polled, Refresh keeps it, and only Check visibility reads it again. Nothing is saved, so
//! a restart forgets it and no file holds remote facts.

use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

use eframe::egui;
use repose_core::host::github::{Visibility, check};

use super::{AppState, Msg};

#[derive(Debug, Default)]
pub struct VisibilityState {
	/// Each remote URL's last answer and when it arrived.
	pub known: HashMap<String, (Visibility, SystemTime)>,
	/// The URLs gh is being asked about right now.
	pub checking: HashSet<String>,
}

impl AppState {
	/// Asks gh about `url` on a thread, unless it is being asked already.
	pub fn check_visibility(&mut self, url: String, ctx: &egui::Context) {
		if !self.visibility.checking.insert(url.clone()) {
			return;
		}
		let tx = self.sender();
		let wake = ctx.clone();
		std::thread::spawn(move || {
			let answer = check(&url);
			let _ = tx.send(Msg::Visibility(url, answer));
			wake.request_repaint();
		});
	}

	pub(super) fn merge_visibility(&mut self, url: String, answer: Visibility) {
		self.visibility.checking.remove(&url);
		self.visibility
			.known
			.insert(url, (answer, SystemTime::now()));
	}
}
