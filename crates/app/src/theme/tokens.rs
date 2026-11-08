//! Colour and size tokens. Values from docs/specs/2026-11-01-repose-v1/technical-spec.md, Theme.
//! Every colour has a dark and a light value under one name (light-theme clause 1). A screen
//! passes the name wherever egui takes `impl Into<Color32>`, and it becomes the value for the
//! theme this thread is drawing, which `set_light` records at the start of each frame.

use std::cell::Cell;

use eframe::egui::Color32;

/// One colour token: its dark-theme and light-theme values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Colour {
	pub dark: Color32,
	pub light: Color32,
}

impl Colour {
	const fn new(dark: [u8; 3], light: [u8; 3]) -> Self {
		Self {
			dark: Color32::from_rgb(dark[0], dark[1], dark[2]),
			light: Color32::from_rgb(light[0], light[1], light[2]),
		}
	}

	/// The value in one theme.
	pub fn of(self, light: bool) -> Color32 {
		if light { self.light } else { self.dark }
	}

	/// The value for the theme this thread is drawing.
	pub fn get(self) -> Color32 {
		self.of(is_light())
	}
}

impl From<Colour> for Color32 {
	fn from(c: Colour) -> Self {
		c.get()
	}
}

thread_local! {
	/// Per thread, so the UI tests running side by side never share a theme. A thread that
	/// never sets it draws dark, as the app did before it had a light theme.
	static LIGHT: Cell<bool> = const { Cell::new(false) };
}

/// Records which theme this thread draws from now on; `theme::begin_frame` calls it every frame.
pub fn set_light(light: bool) {
	LIGHT.with(|l| l.set(light));
}

pub fn is_light() -> bool {
	LIGHT.with(Cell::get)
}

pub const BG_APP: Colour = Colour::new([0x10, 0x15, 0x18], [0xf6, 0xf8, 0xf8]);
pub const BG_PANEL: Colour = Colour::new([0x16, 0x1e, 0x22], [0xee, 0xf2, 0xf2]);
pub const BG_RAISED: Colour = Colour::new([0x18, 0x23, 0x28], [0xff, 0xff, 0xff]);
pub const BG_HOVER: Colour = Colour::new([0x23, 0x31, 0x37], [0xe2, 0xe9, 0xea]);
pub const LINE_SOFT: Colour = Colour::new([0x26, 0x30, 0x36], [0xe0, 0xe6, 0xe7]);
pub const LINE: Colour = Colour::new([0x2b, 0x35, 0x39], [0xd3, 0xdb, 0xdd]);
pub const LINE_STRONG: Colour = Colour::new([0x39, 0x48, 0x4e], [0xb4, 0xc0, 0xc3]);
pub const TEXT: Colour = Colour::new([0xdc, 0xe4, 0xe4], [0x1d, 0x2a, 0x2e]);
pub const TEXT_BRIGHT: Colour = Colour::new([0xe0, 0xeb, 0xe8], [0x0f, 0x1a, 0x1d]);
pub const MUTED: Colour = Colour::new([0x97, 0xa8, 0xad], [0x4e, 0x5f, 0x64]);
pub const ACCENT: Colour = Colour::new([0x87, 0xd6, 0xb2], [0x16, 0x67, 0x4b]);
pub const ACCENT_BG: Colour = Colour::new([0x28, 0x3c, 0x3a], [0xd6, 0xec, 0xe3]);
/// The selected, open or current outline; non-text 3:1 on every fill it sits on in both themes.
pub const ACCENT_LINE: Colour = Colour::new([0x60, 0x8b, 0x7b], [0x3f, 0x8a, 0x6c]);
/// Dark lifted from #809fce so a hash or chevron on the selected-row fill reaches 4.5:1 (audit
/// web-design-guidelines; `tests::text_colours_read_at_aa_on_every_fill_in_both_themes`).
pub const INFO: Colour = Colour::new([0x85, 0xa3, 0xd0], [0x2a, 0x55, 0x90]);
pub const WARN: Colour = Colour::new([0xe3, 0xbe, 0x7b], [0x7a, 0x4d, 0x00]);
pub const DANGER: Colour = Colour::new([0xf0, 0x78, 0x78], [0xa3, 0x24, 0x1d]);
/// Diff line backgrounds, and the stronger fill behind the changed words within a line.
pub const ADDED_BG: Colour = Colour::new([0x17, 0x30, 0x27], [0xe2, 0xf3, 0xe9]);
pub const ADDED_MARK: Colour = Colour::new([0x25, 0x52, 0x3d], [0xc6, 0xe8, 0xd3]);
pub const REMOVED_BG: Colour = Colour::new([0x38, 0x1f, 0x25], [0xfb, 0xe6, 0xe8]);
pub const REMOVED_MARK: Colour = Colour::new([0x5e, 0x2c, 0x35], [0xf6, 0xd2, 0xd7]);

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
/// A list row's monospace name, and its smaller second text: a repository row's branch line, a
/// branch row's short hash.
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

	const TEXTS: [(&str, Colour); 7] = [
		("TEXT", TEXT),
		("TEXT_BRIGHT", TEXT_BRIGHT),
		("MUTED", MUTED),
		("ACCENT", ACCENT),
		("INFO", INFO),
		("WARN", WARN),
		("DANGER", DANGER),
	];
	const FILLS: [(&str, Colour); 9] = [
		("BG_APP", BG_APP),
		("BG_PANEL", BG_PANEL),
		("BG_RAISED", BG_RAISED),
		("BG_HOVER", BG_HOVER),
		("ACCENT_BG", ACCENT_BG),
		("ADDED_BG", ADDED_BG),
		("ADDED_MARK", ADDED_MARK),
		("REMOVED_BG", REMOVED_BG),
		("REMOVED_MARK", REMOVED_MARK),
	];

	/// Light: every text colour on every fill reads at 4.5:1 (light-theme clause 3, computed).
	/// Dark keeps what it promised before the light set: TEXT everywhere, INFO on the selected
	/// fill and the app background. Its other pairs are not all at AA and are not drawn together.
	#[test]
	fn text_colours_read_at_aa_on_every_fill_in_both_themes() {
		for (text, fg) in TEXTS {
			for (fill, bg) in FILLS {
				let ratio = contrast(fg.light, bg.light);
				assert!(ratio >= 4.5, "light {text} on {fill} is {ratio:.2}:1");
			}
		}
		for (fill, bg) in FILLS {
			let ratio = contrast(TEXT.dark, bg.dark);
			assert!(ratio >= 4.5, "dark TEXT on {fill} is {ratio:.2}:1");
		}
		for (fill, bg) in [("ACCENT_BG", ACCENT_BG), ("BG_APP", BG_APP)] {
			let ratio = contrast(INFO.dark, bg.dark);
			assert!(ratio >= 4.5, "dark INFO on {fill} is {ratio:.2}:1");
		}
	}

	/// The selected or open outline (ACCENT_LINE) and the focus ring (ACCENT) are non-text and
	/// need 3:1 (WCAG 1.4.11) against every fill a row or pane can have, in both themes. They stay
	/// distinct colours, so focus never looks like selection (Morticia 5Q-2, #39664).
	#[test]
	fn outlines_read_at_three_to_one_on_every_row_fill_in_both_themes() {
		let fills = &FILLS[..5];
		for (name, line) in [("ACCENT_LINE", ACCENT_LINE), ("ACCENT", ACCENT)] {
			for (fill, bg) in fills {
				for (theme, ratio) in [
					("dark", contrast(line.dark, bg.dark)),
					("light", contrast(line.light, bg.light)),
				] {
					assert!(ratio >= 3.0, "{theme} {name} on {fill} is {ratio:.2}:1");
				}
			}
		}
		assert_ne!(ACCENT_LINE, ACCENT);
	}

	#[test]
	fn a_token_is_the_value_for_the_theme_this_thread_draws() {
		set_light(false);
		assert_eq!(Color32::from(ACCENT), ACCENT.dark);
		set_light(true);
		assert_eq!(Color32::from(ACCENT), ACCENT.light);
		let other = std::thread::spawn(|| Color32::from(ACCENT)).join().ok();
		assert_eq!(other, Some(ACCENT.dark), "another thread starts dark");
		set_light(false);
	}
}
