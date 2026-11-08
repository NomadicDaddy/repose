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

/// A count with its noun: `count_label(1, "file", "files")` is `1 file`, any other count takes the
/// plural. The one place every screen's singular/plural rule lives (Morticia L-1 on c556623).
fn count_label(count: usize, one: &str, many: &str) -> String {
	format!("{count} {}", if count == 1 { one } else { many })
}

/// The colour of a change's kind letter, shared by the Changes badge and the History file rows so
/// a colour is changed in one place.
fn change_kind_colour(kind: ChangeKind) -> crate::theme::Colour {
	match kind {
		ChangeKind::Modified | ChangeKind::TypeChanged => WARN,
		ChangeKind::Added | ChangeKind::IntentToAdd => ACCENT,
		ChangeKind::Deleted | ChangeKind::Conflicted => DANGER,
		ChangeKind::Renamed => INFO,
		ChangeKind::Untracked => MUTED,
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn a_count_takes_the_singular_only_for_one() {
		assert_eq!(super::count_label(1, "file", "files"), "1 file");
		assert_eq!(super::count_label(0, "file", "files"), "0 files");
		assert_eq!(
			super::count_label(12, "unchanged line", "unchanged lines"),
			"12 unchanged lines"
		);
	}
}
