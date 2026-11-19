//! remote-visibility: whether origin is Public, Private or Unknown, read through gh on the user's
//! Check visibility only (A1), and kept in memory per remote with the time it was read. It is
//! never polled, Refresh keeps it, and only Check visibility reads it again. Nothing is saved, so
//! a restart forgets it and no file holds remote facts.
//!
//! A remote is known here only by `display_url` of its URL, never the URL itself: git keeps a
//! configured token in an https URL's userinfo (or an scp query), and this cache outlives the
//! remotes cache that Refresh clears (Roger 5Q-3 on db5d258). gh is given the same credential-free
//! form, which names the same owner and repository.

use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

use eframe::egui;
use repose_core::git::remote::display_url;
use repose_core::host::github::{Visibility, check};

use super::{AppState, Msg};

#[derive(Debug, Default)]
pub struct VisibilityState {
	/// Each remote's last answer and when it arrived, by `display_url`.
	pub(super) known: HashMap<String, (Visibility, SystemTime)>,
	/// The remotes gh is being asked about right now, by `display_url`.
	pub(super) checking: HashSet<String>,
}

impl VisibilityState {
	/// The last answer for the remote at `url`, and when it arrived.
	pub fn answer(&self, url: &str) -> Option<&(Visibility, SystemTime)> {
		self.known.get(&display_url(url))
	}

	/// Whether gh is being asked about the remote at `url` right now.
	pub fn is_checking(&self, url: &str) -> bool {
		self.checking.contains(&display_url(url))
	}
}

impl AppState {
	/// Asks gh about the remote at `url` on a thread, unless it is being asked already.
	pub fn check_visibility(&mut self, url: &str, ctx: &egui::Context) {
		let remote = display_url(url);
		if !self.visibility.checking.insert(remote.clone()) {
			return;
		}
		let tx = self.sender();
		let wake = ctx.clone();
		std::thread::spawn(move || {
			let answer = check(&remote);
			let _ = tx.send(Msg::Visibility(remote, answer));
			wake.request_repaint();
		});
	}

	/// `remote` is the `display_url` the check was started with.
	pub(super) fn merge_visibility(&mut self, remote: String, answer: Visibility) {
		self.visibility.checking.remove(&remote);
		self.visibility
			.known
			.insert(remote, (answer, SystemTime::now()));
	}
}
