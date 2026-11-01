//! Segoe UI for interface text and Cascadia Code for names, hashes and code, read from the
//! Windows font folder when present. egui's bundled fonts remain as fallbacks.

use std::sync::Arc;

use eframe::egui::{FontData, FontDefinitions, FontFamily};

const CANDIDATES: [(&str, &str, bool); 4] = [
	("segoe-ui", "C:/Windows/Fonts/segoeui.ttf", false),
	("cascadia-code", "C:/Windows/Fonts/CascadiaCode.ttf", true),
	("cascadia-mono", "C:/Windows/Fonts/CascadiaMono.ttf", true),
	("consolas", "C:/Windows/Fonts/consola.ttf", true),
];

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
