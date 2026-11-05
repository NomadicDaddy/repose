//! The screens. Each file draws one pane from `AppState` and never owns state of its own.

mod a11y;
pub mod branches;
pub mod changes;
mod close_guard;
pub mod commit_box;
pub mod confirm;
mod crumb;
pub mod detail;
pub mod diff;
mod diff_draw;
mod fetch;
pub mod file_view;
pub mod files;
pub mod footer;
pub mod history;
mod menu;
pub mod repo_list;
mod row;
mod status_line;

use repose_core::model::ChangeKind;

use crate::theme::{ACCENT, DANGER, INFO, MUTED, WARN};

/// The colour of a change's kind letter, shared by the Changes badge and the History file rows so
/// a colour is changed in one place.
fn change_kind_colour(kind: ChangeKind) -> eframe::egui::Color32 {
	match kind {
		ChangeKind::Modified | ChangeKind::TypeChanged => WARN,
		ChangeKind::Added | ChangeKind::IntentToAdd => ACCENT,
		ChangeKind::Deleted | ChangeKind::Conflicted => DANGER,
		ChangeKind::Renamed => INFO,
		ChangeKind::Untracked => MUTED,
	}
}
