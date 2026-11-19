//! remote-visibility on screen: the repository menu's Check visibility, and the header's badge
//! for origin once it has been checked. The badge names no URL, so display_url's rule holds.

use std::path::Path;

use eframe::egui::{RichText, Ui};
use repose_core::git::remote::origin;
use repose_core::host::github::Visibility;

use super::{a11y, status_line::clock};
use crate::state::AppState;
use crate::theme::*;

pub(super) const CHECK: &str = "Check visibility";
const CHECKING: &str = "Checking visibility…";

/// Why Check visibility is off for origin's `url` right now, or None when it can run.
pub(super) fn blocked(state: &AppState, url: &str) -> Option<&'static str> {
	state.visibility.is_checking(url).then_some(CHECKING)
}

/// "Visibility: Public", "Private" or "Unknown" for the repository's origin, with the time it was
/// read (and, for Unknown, why) as its tooltip; "Checking visibility…" while the first answer is
/// awaited; nothing before the user has checked.
pub(super) fn badge(ui: &mut Ui, state: &AppState, repo: &Path) {
	let Some(Ok(list)) = state.remotes.get(repo) else {
		return;
	};
	let Some(url) = origin(list).map(|entry| entry.url.as_str()) else {
		return;
	};
	let checking = state.visibility.is_checking(url);
	let Some((answer, at)) = state.visibility.answer(url) else {
		if checking {
			a11y::loading_label(ui, CHECKING);
		}
		return;
	};
	let when = clock(*at, state.utc_offset);
	let (word, colour, tip) = match answer {
		Visibility::Public => (
			"Public",
			INFO,
			format!("Read {when} from GitHub through gh"),
		),
		Visibility::Private => (
			"Private",
			ACCENT,
			format!("Read {when} from GitHub through gh"),
		),
		Visibility::Unknown(why) => ("Unknown", MUTED, format!("{why} (checked {when})")),
	};
	let line = if checking {
		format!("Visibility: {word} · checking again…")
	} else {
		format!("Visibility: {word}")
	};
	let shown = ui.label(RichText::new(&line).size(TEXT_SMALL).color(colour));
	a11y::live(&shown, &line);
	shown.on_hover_text(tip);
}
