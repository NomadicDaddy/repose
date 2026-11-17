//! Segoe UI for interface text and Cascadia Code for names, hashes and code, read from the
//! Windows font folder when present. egui's bundled fonts remain as fallbacks.

use std::sync::Arc;

use eframe::egui::{
	self, FontData, FontDefinitions, FontFamily, FontInsert, FontPriority, InsertFontFamily,
};

const CANDIDATES: [(&str, &str, bool); 4] = [
	("segoe-ui", "C:/Windows/Fonts/segoeui.ttf", false),
	("cascadia-code", "C:/Windows/Fonts/CascadiaCode.ttf", true),
	("cascadia-mono", "C:/Windows/Fonts/CascadiaMono.ttf", true),
	("consolas", "C:/Windows/Fonts/consola.ttf", true),
];

/// The family Activity's loading glyphs draw in (activity-loading-glyphs): Yu Gothic Medium, for
/// its digits and half-width katakana. Not MS Gothic: its embedded bitmaps (EBDT) make egui treat
/// every glyph as a colour bitmap and paint it white, ignoring the tint (the 27f450a captures,
/// after the override colour had fixed the f1e55af ones).
pub const GLYPH_FAMILY: &str = "activity-glyphs";
const GLYPH_FONT: &str = "C:/Windows/Fonts/YuGothM.ttc";

/// Adds the glyph family. It is read only when Activity first loads, so a launch that never shows
/// the glyphs never reads the 14 MB font, and on its own thread, so the first glyph frame does
/// not wait on the disk (Morticia, note 2 on 23fdba0); until it lands, and without the file, the
/// glyphs are digits.
pub fn add_glyph_font(ctx: &egui::Context) {
	let ctx = ctx.clone();
	let _ = std::thread::Builder::new()
		.name("glyph-font".into())
		.spawn(move || {
			let Ok(bytes) = std::fs::read(GLYPH_FONT) else {
				return;
			};
			let family = InsertFontFamily {
				family: FontFamily::Name(GLYPH_FAMILY.into()),
				priority: FontPriority::Highest,
			};
			let data = FontData::from_owned(bytes);
			ctx.add_font(FontInsert::new(GLYPH_FAMILY, data, vec![family]));
			ctx.request_repaint();
		});
}

pub fn definitions() -> FontDefinitions {
	let mut defs = FontDefinitions::default();
	let mut mono_added = false;
	for (key, path, mono) in CANDIDATES {
		if mono && mono_added {
			continue;
		}
		let Ok(bytes) = std::fs::read(path) else {
			continue;
		};
		defs.font_data
			.insert(key.to_string(), Arc::new(FontData::from_owned(bytes)));
		let family = if mono {
			FontFamily::Monospace
		} else {
			FontFamily::Proportional
		};
		defs.families
			.entry(family)
			.or_default()
			.insert(0, key.to_string());
		if mono {
			mono_added = true;
		}
	}
	defs
}
