//! Colour and size tokens. Values from docs/specs/2026-11-01-repose-v1/technical-spec.md, Theme.

use eframe::egui::Color32;

pub const BG_APP: Color32 = Color32::from_rgb(0x10, 0x15, 0x18);
pub const BG_PANEL: Color32 = Color32::from_rgb(0x16, 0x1e, 0x22);
pub const BG_RAISED: Color32 = Color32::from_rgb(0x18, 0x23, 0x28);
pub const BG_HOVER: Color32 = Color32::from_rgb(0x23, 0x31, 0x37);
pub const LINE_SOFT: Color32 = Color32::from_rgb(0x26, 0x30, 0x36);
pub const LINE: Color32 = Color32::from_rgb(0x2b, 0x35, 0x39);
pub const LINE_STRONG: Color32 = Color32::from_rgb(0x39, 0x48, 0x4e);
pub const TEXT: Color32 = Color32::from_rgb(0xdc, 0xe4, 0xe4);
pub const TEXT_BRIGHT: Color32 = Color32::from_rgb(0xe0, 0xeb, 0xe8);
pub const MUTED: Color32 = Color32::from_rgb(0x97, 0xa8, 0xad);
pub const ACCENT: Color32 = Color32::from_rgb(0x87, 0xd6, 0xb2);
pub const ACCENT_BG: Color32 = Color32::from_rgb(0x28, 0x3c, 0x3a);
pub const ACCENT_LINE: Color32 = Color32::from_rgb(0x60, 0x8b, 0x7b);
/// Lifted from #809fce so a hash or chevron on the selected-row fill reaches 4.5:1 (audit
/// web-design-guidelines; `tests::info_reads_at_aa_on_the_selected_fill_and_the_app_background`).
pub const INFO: Color32 = Color32::from_rgb(0x85, 0xa3, 0xd0);
pub const WARN: Color32 = Color32::from_rgb(0xe3, 0xbe, 0x7b);
pub const DANGER: Color32 = Color32::from_rgb(0xf0, 0x78, 0x78);
/// Diff line backgrounds, and the stronger fill behind the changed words within a line.
pub const ADDED_BG: Color32 = Color32::from_rgb(0x17, 0x30, 0x27);
pub const ADDED_MARK: Color32 = Color32::from_rgb(0x25, 0x52, 0x3d);
pub const REMOVED_BG: Color32 = Color32::from_rgb(0x38, 0x1f, 0x25);
pub const REMOVED_MARK: Color32 = Color32::from_rgb(0x5e, 0x2c, 0x35);

/// Base interface text size in points at 100% scaling.
pub const TEXT_BASE: f32 = 13.0;
/// The smallest text drawn anywhere: section labels and badges.
pub const TEXT_SMALL: f32 = 11.0;
pub const TEXT_TITLE: f32 = 17.0;
/// The selected repository's name at the top of the detail pane.
pub const TEXT_DETAIL_TITLE: f32 = 18.0;
/// Metadata lines under a title: the detail header's branch, path and upstream.
pub const TEXT_META: f32 = 12.0;
/// Monospace text in lists and diffs: a commit's hash and author line, a diff or file line.
pub const TEXT_MONO: f32 = 12.0;
/// A repository row's name (monospace) and the branch line under it.
pub const TEXT_ROW_NAME: f32 = 14.0;
pub const TEXT_ROW_BRANCH: f32 = 11.5;
/// A repository row in the left list.
pub const ROW_HEIGHT: f32 = 46.0;
/// A commit row in History and a branch row in Branches; separate tokens though equal today.
pub const COMMIT_ROW_HEIGHT: f32 = 32.0;
pub const BRANCH_ROW_HEIGHT: f32 = 32.0;
/// A file row in the Changes groups.
pub const CHANGE_ROW_HEIGHT: f32 = 30.0;
/// One line of a diff or of a file read at a revision.
pub const DIFF_ROW_HEIGHT: f32 = 20.0;
pub const SIDEBAR_WIDTH: f32 = 300.0;
pub const RADIUS: f32 = 4.0;

#[cfg(test)]
mod tests {
	use super::*;
	use crate::theme::contrast::contrast;

	/// The sizes the screens used as literals before they were named (the theme-token tidy-up);
	/// naming them changed no value, and changing one is a visual change, not a refactor.
	#[test]
	fn the_named_sizes_keep_the_values_the_screens_drew() {
		assert_eq!(
			[
				TEXT_DETAIL_TITLE,
				TEXT_META,
				TEXT_MONO,
				TEXT_ROW_NAME,
				TEXT_ROW_BRANCH
			],
			[18.0, 12.0, 12.0, 14.0, 11.5]
		);
		assert_eq!(
			[
				ROW_HEIGHT,
				COMMIT_ROW_HEIGHT,
				BRANCH_ROW_HEIGHT,
				CHANGE_ROW_HEIGHT,
				DIFF_ROW_HEIGHT
			],
			[46.0, 32.0, 32.0, 30.0, 20.0]
		);
	}

	#[test]
	fn info_reads_at_aa_on_the_selected_fill_and_the_app_background() {
		for (name, bg) in [("ACCENT_BG", ACCENT_BG), ("BG_APP", BG_APP)] {
			let ratio = contrast(INFO, bg);
			assert!(ratio >= 4.5, "INFO on {name} is {ratio:.2}:1");
		}
	}
}
