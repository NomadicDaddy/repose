//! The bottom row: build information at the right, as Summon's status bar carries it (Phillip
//! #31631). The version is the crate's, the commit and build time are stamped by build.rs.

use eframe::egui::{self, RichText, Ui};

use crate::theme::*;

pub const FOOTER_HEIGHT: f32 = 22.0;

/// `repose 0.1.0 · 35818fb · built 2026-11-02 09:40 UTC`; a build outside a checkout reads
/// `unknown` for the commit and one with uncommitted tracked changes carries `+dirty`.
pub fn build_info() -> String {
	format!(
		"repose {} · {} · built {}",
		env!("CARGO_PKG_VERSION"),
		env!("REPOSE_COMMIT"),
		env!("REPOSE_BUILT_AT")
	)
}

pub fn show(root: &mut Ui) {
	egui::Panel::bottom("footer")
		.exact_size(FOOTER_HEIGHT)
		.resizable(false)
		.frame(
			egui::Frame::new()
				.fill(BG_PANEL)
				.stroke(egui::Stroke::new(1.0, LINE_SOFT))
				.inner_margin(egui::Margin::symmetric(10, 0)),
		)
		.show(root, |ui| {
			ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
				ui.label(RichText::new(build_info()).size(TEXT_SMALL).color(MUTED));
			});
		});
}
