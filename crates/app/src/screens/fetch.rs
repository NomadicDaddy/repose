//! What Fetch shows (remote-sync, A1 as revised by Phillip #35530): the refs note beside a
//! repository's branch line, the Fetch button in the History header, and a failed fetch under the
//! repository header with git's own message. The fetch itself runs in the write queue.

use std::path::Path;

use eframe::egui::{self, FontFamily, RichText, Ui};

use super::{a11y, status_line::clock};
use crate::state::{AppState, WriteKind};
use crate::theme::*;

/// "Fetching…" while a fetch is queued or running, "Fetched HH:MM" after one succeeded, and
/// otherwise "Saved refs; no fetch": the counts come from the refs on disk.
pub(super) fn refs_note(state: &AppState, repo: &Path) -> String {
	if state.writes.fetching(repo) {
		"Fetching…".to_string()
	} else if let Some(at) = state.fetched.get(repo) {
		format!("Fetched {}", clock(*at, state.utc_offset))
	} else {
		"Saved refs; no fetch".to_string()
	}
}

/// The list-wide part of the status line: no fetch at all this session, or how many repositories
/// were fetched and when the last one finished. Each repository's own line says when it was.
pub(super) fn list_note(state: &AppState) -> String {
	match state.fetched.values().max() {
		None => "Saved refs; no fetch".to_string(),
		Some(last) => format!(
			"Saved refs; {} fetched, last {}",
			state.fetched.len(),
			clock(*last, state.utc_offset)
		),
	}
}

/// Why Fetch cannot run for this repository now, or None when it can.
pub(super) fn blocked(state: &AppState, repo: &Path) -> Option<String> {
	if state.writes.fetching(repo) {
		return Some("Fetching…".into());
	}
	match state.remotes.get(repo) {
		None => Some("Reading the remotes…".into()),
		Some(Ok(list)) if list.is_empty() => Some("This repository has no remote".into()),
		Some(Err(e)) => Some(format!("The remotes could not be read: {e}")),
		Some(Ok(_)) => None,
	}
}

/// The Fetch button for a pane header: on a click, the fetch is queued for this repository.
pub(super) fn button(ui: &mut Ui, state: &mut AppState, repo: &Path) {
	match blocked(state, repo) {
		Some(why) => {
			ui.add_enabled(false, egui::Button::new("Fetch"))
				.on_disabled_hover_text(why);
		}
		None => {
			let clicked = ui
				.button("Fetch")
				.on_hover_text("Fetch from the remote; nothing else ever reaches the network")
				.clicked();
			if clicked {
				let ctx = ui.ctx().clone();
				state.request_write(repo.to_path_buf(), WriteKind::Fetch, &ctx);
			}
		}
	}
}

/// The last fetch's failure for this repository, headed "Fetch failed" (or "Fetch cancelled")
/// with git's message, which is where a sign-in request or a network error is explained.
pub(super) fn outcome(ui: &mut Ui, state: &AppState, repo: &Path) {
	let Some(last) = state.writes.last.get(repo) else {
		return;
	};
	if last.kind != WriteKind::Fetch || state.writes.fetching(repo) {
		return;
	}
	let Some(error) = &last.error else {
		return;
	};
	let what = if last.cancelled {
		"Fetch cancelled"
	} else {
		"Fetch failed"
	};
	let shown = ui.label(RichText::new(what).color(DANGER).strong());
	a11y::live(&shown, what);
	egui::ScrollArea::vertical()
		.id_salt(("fetch-error", repo))
		.max_height(120.0)
		.auto_shrink([false, true])
		.show(ui, |ui| {
			ui.label(
				RichText::new(error)
					.size(TEXT_SMALL)
					.color(TEXT)
					.family(FontFamily::Monospace),
			);
		});
}
