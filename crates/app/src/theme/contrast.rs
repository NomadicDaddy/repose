//! WCAG 2 contrast, and the guard that keeps syntax colours readable on every background a span
//! is drawn over (audit-web-design-guidelines-1793709090: the Base16 Ocean Dark colours were
//! copied without checking them against the diff and word-mark fills). Each theme is checked
//! against its own fills; the caller names the theme, because the syntax colours are worked out
//! on the reader thread, which draws nothing.

use eframe::egui::Color32;

use super::tokens::{
	ACCENT_BG, ADDED_BG, ADDED_MARK, BG_APP, BG_PANEL, BG_RAISED, Colour, REMOVED_BG, REMOVED_MARK,
	TEXT,
};

/// The AA ratio for normal-size text.
pub const AA: f64 = 4.5;

/// The ratio the syntax guard lifts to: AA plus a 0.1 margin, so a lifted colour never sits
/// exactly at the AA line. The 92a14a6 light captures, taken with this margin, measured the
/// tightest span at 4.66:1 on REMOVED_MARK.
pub const LIFT: f64 = AA + 0.1;

/// Every fill a syntax-coloured span can sit on: the panes, the current file line, added and
/// removed lines, and the word marks inside them.
pub const SPAN_BACKGROUNDS: [(&str, Colour); 8] = [
	("BG_APP", BG_APP),
	("BG_PANEL", BG_PANEL),
	("BG_RAISED", BG_RAISED),
	("ACCENT_BG", ACCENT_BG),
	("ADDED_BG", ADDED_BG),
	("ADDED_MARK", ADDED_MARK),
	("REMOVED_BG", REMOVED_BG),
	("REMOVED_MARK", REMOVED_MARK),
];

/// WCAG 2 relative luminance of an sRGB colour.
pub fn luminance(c: Color32) -> f64 {
	let lin = |v: u8| {
		let v = f64::from(v) / 255.0;
		if v <= 0.04045 {
			v / 12.92
		} else {
			((v + 0.055) / 1.055).powf(2.4)
		}
	};
	0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

pub fn contrast(a: Color32, b: Color32) -> f64 {
	let (x, y) = (luminance(a), luminance(b));
	(x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// The lowest contrast `c` has against any of one theme's span backgrounds.
pub fn worst(c: Color32, light: bool) -> f64 {
	SPAN_BACKGROUNDS
		.iter()
		.map(|(_, bg)| contrast(c, bg.of(light)))
		.fold(f64::INFINITY, f64::min)
}

/// `c` itself when it reads at LIFT on every span background of the theme; otherwise the least
/// blend toward that theme's TEXT that does, in 5% steps, so a failing colour keeps as much of its
/// hue as it can.
pub fn readable(c: Color32, light: bool) -> Color32 {
	let text = TEXT.of(light);
	for step in 0..=20u8 {
		let t = f32::from(step) / 20.0;
		let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
		let candidate = Color32::from_rgb(
			mix(c.r(), text.r()),
			mix(c.g(), text.g()),
			mix(c.b(), text.b()),
		);
		if worst(candidate, light) >= LIFT {
			return candidate;
		}
	}
	text
}

#[cfg(test)]
mod tests {
	use super::{AA, LIFT, SPAN_BACKGROUNDS, contrast, readable, worst};
	use crate::highlight::colour_lines;
	use crate::theme::tokens::TEXT;
	use eframe::egui::Color32;

	/// The theme's comment grey as Base16 Ocean Dark ships it, before the guard.
	const OCEAN_COMMENT: Color32 = Color32::from_rgb(0x65, 0x73, 0x7e);

	#[test]
	fn the_shipped_comment_grey_fails_and_the_guard_lifts_it() {
		assert!(
			worst(OCEAN_COMMENT, false) < AA,
			"control: the raw grey fails somewhere"
		);
		let fixed = readable(OCEAN_COMMENT, false);
		assert!(
			worst(fixed, false) >= LIFT,
			"{fixed:?} is {:.2}:1 at worst",
			worst(fixed, false)
		);
		assert_ne!(fixed, OCEAN_COMMENT);
		for light in [false, true] {
			let text = TEXT.of(light);
			assert_eq!(
				readable(text, light),
				text,
				"a colour that already reads is left alone"
			);
		}
	}

	#[test]
	fn every_real_span_reads_at_the_lift_ratio_on_every_background_in_both_themes() {
		let lines = [
			"// a comment",
			"/// a doc comment",
			"fn main() { let s = \"text\"; let n = 42; if n > 1 { return; } }",
			"use std::path::Path; struct Row<'a> { name: &'a str }",
		];
		let spans = colour_lines("lib.rs", &lines);
		let mut checked = 0;
		for line in &spans {
			for span in line {
				for (name, bg) in SPAN_BACKGROUNDS {
					for (theme, fg, bg) in [
						("dark", span.color.dark, bg.dark),
						("light", span.color.light, bg.light),
					] {
						let ratio = contrast(fg, bg);
						assert!(ratio >= LIFT, "{theme} {fg:?} on {name} is {ratio:.2}:1");
					}
				}
				checked += 1;
			}
		}
		assert!(checked >= 8, "the fixture produced real spans ({checked})");
		// Recognition is unchanged: the comment and the keyword still get different colours, in
		// each theme.
		let comment = spans[0].first().map(|s| s.color);
		let keyword = spans[2].first().map(|s| s.color);
		assert!(comment.is_some() && keyword.is_some());
		assert!(comment.map(|c| c.dark) != keyword.map(|c| c.dark));
		assert!(comment.map(|c| c.light) != keyword.map(|c| c.light));
	}
}
