//! remote-visibility: whether origin is Public, Private or Unknown, read through gh on the user's
//! Check visibility only (A1), and kept in memory per remote with the time it was read. It is
//! never polled, Refresh keeps it, and only Check visibility reads it again. Nothing is saved, so
//! a restart forgets it and no file holds remote facts.
//!
//! The origin URL is checked as given, on the UI thread, before anything runs: only a URL that
//! `repository` accepts sends its checked owner and repository to the worker (Roger 5Q-3 on
//! b551265). The cache never holds the URL itself, since git keeps a configured token in an https
//! URL's userinfo or an scp query and this cache outlives the remotes cache Refresh clears (Roger
//! 5Q-3 on db5d258). An accepted origin is known by its GitHub owner and repository, a refused one
//! by its display_url under a separate prefix, so the two never share an answer.

use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

use eframe::egui;
use repose_core::git::remote::display_url;
use repose_core::host::github::{Visibility, ask, repository};

use super::{AppState, Msg};

#[derive(Debug, Default)]
pub struct VisibilityState {
	/// Each remote's last answer and when it arrived, by `remote_key`.
	pub(super) known: HashMap<String, (Visibility, SystemTime)>,
	/// The remotes gh is being asked about right now, by `remote_key`.
	pub(super) checking: HashSet<String>,
}

/// How a remote is known here, which is never its raw URL, and its checked owner and repository,
/// or why `repository` refused it.
pub(super) fn remote_key(url: &str) -> (String, Result<(String, String), &'static str>) {
	let checked = repository(url);
	let key = match &checked {
		Ok((owner, repo)) => format!("github.com/{owner}/{repo}"),
		Err(_) => format!("refused {}", display_url(url)),
	};
	(key, checked)
}

impl VisibilityState {
	/// The last answer for the remote at `url`, and when it arrived.
	pub fn answer(&self, url: &str) -> Option<&(Visibility, SystemTime)> {
		self.known.get(&remote_key(url).0)
	}

	/// Whether gh is being asked about the remote at `url` right now.
	pub fn is_checking(&self, url: &str) -> bool {
		self.checking.contains(&remote_key(url).0)
	}
}

impl AppState {
	/// Checks origin's `url` as given. A refused one is Unknown with why at once and nothing runs;
	/// an accepted one asks gh about its owner and repository on a thread, unless it is being
	/// asked already.
	pub fn check_visibility(&mut self, url: &str, ctx: &egui::Context) {
		let (key, checked) = remote_key(url);
		let (owner, repo) = match checked {
			Ok(names) => names,
			Err(why) => {
				self.merge_visibility(key, Visibility::Unknown(why.to_string()));
				return;
			}
		};
		if !self.visibility.checking.insert(key.clone()) {
			return;
		}
		let tx = self.sender();
		let wake = ctx.clone();
		std::thread::spawn(move || {
			let answer = ask(&owner, &repo);
			let _ = tx.send(Msg::Visibility(key, answer));
			wake.request_repaint();
		});
	}

	/// `key` is the `remote_key` the check was started with.
	pub(super) fn merge_visibility(&mut self, key: String, answer: Visibility) {
		self.visibility.checking.remove(&key);
		self.visibility
			.known
			.insert(key, (answer, SystemTime::now()));
	}
}
