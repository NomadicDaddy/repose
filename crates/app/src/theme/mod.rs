//! The dark theme: tokens from look.jpg and the two fonts. Screens use these names and never a
//! colour literal.

pub mod fonts;
pub mod tokens;

use eframe::egui;

pub use tokens::*;

pub fn apply(ctx: &egui::Context) {
	ctx.set_fonts(fonts::definitions());
	let mut visuals = egui::Visuals::dark();
	visuals.panel_fill = BG_APP;
	visuals.window_fill = BG_PANEL;
	visuals.extreme_bg_color = BG_APP;
	visuals.faint_bg_color = BG_RAISED;
	visuals.override_text_color = Some(TEXT);
	visuals.selection.bg_fill = ACCENT_BG;
	visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT_LINE);
	visuals.hyperlink_color = ACCENT;
	visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, LINE_SOFT);
	visuals.widgets.noninteractive.bg_fill = BG_PANEL;
	visuals.widgets.inactive.bg_fill = BG_RAISED;
	visuals.widgets.inactive.weak_bg_fill = BG_RAISED;
	visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, LINE);
	visuals.widgets.hovered.bg_fill = BG_HOVER;
	visuals.widgets.hovered.weak_bg_fill = BG_HOVER;
	visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, LINE_STRONG);
	visuals.widgets.active.bg_fill = ACCENT_BG;
	visuals.widgets.active.weak_bg_fill = ACCENT_BG;
	visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, ACCENT_LINE);
	ctx.set_theme(egui::Theme::Dark);
	ctx.set_visuals(visuals);
	ctx.all_styles_mut(|style| {
		style.spacing.item_spacing = egui::vec2(8.0, 6.0);
		style.spacing.button_padding = egui::vec2(10.0, 5.0);
	});
}
