//! WCAG 2 contrast, and the guard that keeps syntax colours readable on every background a span
//! is drawn over (audit-web-design-guidelines-1793709090: the Base16 Ocean Dark colours were
//! copied without checking them against the diff and word-mark fills).

use eframe::egui::Color32;

use super::tokens::{
	ACCENT_BG, ADDED_BG, ADDED_MARK, BG_APP, BG_PANEL, BG_RAISED, REMOVED_BG, REMOVED_MARK, TEXT,
};

/// The AA ratio for normal-size text.
pub const AA: f64 = 4.5;

/// Every fill a syntax-coloured span can sit on: the panes, the current file line, added and
/// removed lines, and the word marks inside them.
pub const SPAN_BACKGROUNDS: [(&str, Color32); 8] = [
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

/// The lowest contrast `c` has against any span background.
pub fn worst(c: Color32) -> f64 {
	SPAN_BACKGROUNDS
		.iter()
		.map(|(_, bg)| contrast(c, *bg))
		.fold(f64::INFINITY, f64::min)
}

/// `c` itself when it reads at AA on every span background; otherwise the least blend toward
/// TEXT that does, in 5% steps, so a failing colour keeps as much of its hue as it can.
pub fn readable(c: Color32) -> Color32 {
	for step in 0..=20u8 {
		let t = f32::from(step) / 20.0;
		let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
		let candidate = Color32::from_rgb(
			mix(c.r(), TEXT.r()),
			mix(c.g(), TEXT.g()),
			mix(c.b(), TEXT.b()),
		);
		if worst(candidate) >= AA {
			return candidate;
		}
	}
	TEXT
}

#[cfg(test)]
mod tests {
	use super::{AA, SPAN_BACKGROUNDS, contrast, readable, worst};
	use crate::highlight::colour_lines;
	use crate::theme::tokens::TEXT;
	use eframe::egui::Color32;

	/// The theme's comment grey as Base16 Ocean Dark ships it, before the guard.
	const OCEAN_COMMENT: Color32 = Color32::from_rgb(0x65, 0x73, 0x7e);

	#[test]
	fn the_shipped_comment_grey_fails_and_the_guard_lifts_it() {
		assert!(
			worst(OCEAN_COMMENT) < AA,
			"control: the raw grey fails somewhere"
		);
		let fixed = readable(OCEAN_COMMENT);
		assert!(
			worst(fixed) >= AA,
			"{fixed:?} is {:.2}:1 at worst",
			worst(fixed)
		);
		assert_ne!(fixed, OCEAN_COMMENT);
		assert_eq!(
			readable(TEXT),
			TEXT,
			"a colour that already reads is left alone"
		);
	}

	#[test]
	fn every_real_span_reads_at_aa_on_every_background() {
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
					let ratio = contrast(span.color, bg);
					assert!(ratio >= AA, "{:?} on {name} is {ratio:.2}:1", span.color);
				}
				checked += 1;
			}
		}
		assert!(checked >= 8, "the fixture produced real spans ({checked})");
		// Recognition is unchanged: the comment and the keyword still get different colours.
		let comment = spans[0].first().map(|s| s.color);
		let keyword = spans[2].first().map(|s| s.color);
		assert!(comment.is_some() && keyword.is_some() && comment != keyword);
	}
}
