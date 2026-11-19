//! ai-commit-message-suggestions: Suggest message asks the installed Claude Code for a message for
//! the staged changes, on the user's press only (A1). The staged diff is read and sent on a thread;
//! nothing runs on focus, typing, staging, a refresh or the watcher. The answer fills an empty
//! message at once, or waits for the user to agree before it replaces what they typed, and it is
//! never committed by anything but their own Commit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui;
use repose_core::config::SuggestMessages;
use repose_core::git::Cancel;
use repose_core::git::diff::{staged_diff_for_suggestion, staged_files};
use repose_core::suggest::{CANCELLED, Staged, Suggestion, suggest};

use super::{AppState, Msg};

/// What a repository's last suggestion left to show under its message box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestNote {
	/// It worked, written without these files' diffs (over the cap).
	LeftOut(Vec<String>),
	/// It failed, or was cancelled, for this reason.
	Failed(String),
}

#[derive(Debug, Default)]
pub struct SuggestState {
	/// The repositories a suggestion is running for, with the Cancel that stops it.
	pub(super) running: HashMap<PathBuf, Cancel>,
	/// A suggestion that arrived while the user's own message was in the box, until they choose.
	pub pending: HashMap<PathBuf, String>,
	/// Each repository's note from its last suggestion.
	pub notes: HashMap<PathBuf, SuggestNote>,
}

impl SuggestState {
	pub fn is_running(&self, repo: &Path) -> bool {
		self.running.contains_key(repo)
	}
}

impl AppState {
	/// Whether Settings has a provider picked, so the commit box offers Suggest message.
	pub fn suggestions_on(&self) -> bool {
		self.config.suggest_messages != SuggestMessages::Off
	}

	/// Starts a suggestion for `repo`'s staged changes, unless suggestions are off or one is
	/// running for it already. Its earlier note and any unanswered suggestion are dropped.
	pub fn suggest_message(&mut self, repo: &Path, ctx: &egui::Context) {
		if !self.suggestions_on() || self.suggest.is_running(repo) {
			return;
		}
		self.suggest.notes.remove(repo);
		self.suggest.pending.remove(repo);
		let cancel = Cancel::new();
		self.suggest
			.running
			.insert(repo.to_path_buf(), cancel.clone());
		let tx = self.sender();
		let wake = ctx.clone();
		let repo = repo.to_path_buf();
		std::thread::spawn(move || {
			let staged = staged_files(&repo).and_then(|files| {
				let diff = staged_diff_for_suggestion(&repo)?;
				Ok(Staged { files, diff })
			});
			let result = match staged {
				Ok(staged) => suggest(&staged, &cancel),
				Err(e) => Err(format!("The staged changes could not be read: {e}")),
			};
			let _ = tx.send(Msg::Suggested(repo, result));
			wake.request_repaint();
		});
	}

	/// The user's Cancel: stops the running suggestion for `repo` and everything it started.
	pub fn cancel_suggestion(&mut self, repo: &Path) {
		if let Some(cancel) = self.suggest.running.get(repo) {
			cancel.cancel();
		}
	}

	/// A suggestion came back. It fills an empty message; over the user's own text it waits for
	/// their answer. A failure leaves the message as it was and says why. After the user's Cancel
	/// an answer is dropped, however it ended (Roger 5Q-1 on d3da4dd).
	pub(super) fn merge_suggestion(&mut self, repo: PathBuf, result: Result<Suggestion, String>) {
		let cancelled = self
			.suggest
			.running
			.remove(&repo)
			.is_some_and(|c| c.is_cancelled());
		let result = match result {
			Ok(_) if cancelled => Err(CANCELLED.to_string()),
			other => other,
		};
		let Suggestion { message, left_out } = match result {
			Ok(s) => s,
			Err(why) => {
				self.suggest.notes.insert(repo, SuggestNote::Failed(why));
				return;
			}
		};
		if !left_out.is_empty() {
			self.suggest
				.notes
				.insert(repo.clone(), SuggestNote::LeftOut(left_out));
		}
		let draft = self.commit_messages.entry(repo.clone()).or_default();
		if draft.trim().is_empty() {
			*draft = message;
		} else {
			self.suggest.pending.insert(repo, message);
		}
	}

	/// The user's answer to "replace your message?": the suggestion takes the box, or is dropped.
	pub fn answer_suggestion(&mut self, repo: &Path, replace: bool) {
		if let Some(message) = self.suggest.pending.remove(repo)
			&& replace
		{
			self.commit_messages.insert(repo.to_path_buf(), message);
		}
	}
}
