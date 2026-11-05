//! The dark theme: tokens from look.jpg and the two fonts. Screens use these names and never a
//! colour literal.

pub mod contrast;
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

/// How long after the first rows the animation setting is asked for, so its pwsh starts after
/// the first status and diff reads rather than during them.
const MOTION_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

/// Reads the desktop's animation setting once, off the UI thread and `MOTION_DELAY` after the
/// call, and turns egui's fades and smooth scrolling off when the user has turned animation
/// effects off. Until the answer lands, and when it cannot be read, egui's defaults stand. There
/// is no setting of repose's own.
pub fn follow_reduced_motion(ctx: &egui::Context) {
	let ctx = ctx.clone();
	let _ = std::thread::Builder::new()
		.name("reduced-motion".into())
		.spawn(move || {
			std::thread::sleep(MOTION_DELAY);
			if crate::platform::animations_enabled() == Some(false) {
				ctx.all_styles_mut(|style| set_motion(style, false));
				ctx.request_repaint();
			}
		});
}

/// With animations off: no fade (animation_time 0) and scrolling that jumps instead of gliding.
/// With them on the style is left as it is.
pub fn set_motion(style: &mut egui::Style, animations: bool) {
	if !animations {
		style.animation_time = 0.0;
		style.scroll_animation = egui::style::ScrollAnimation::none();
	}
}

#[cfg(test)]
mod tests {
	use eframe::egui::{self, style::ScrollAnimation};

	use super::set_motion;

	#[test]
	fn animation_effects_off_stops_fades_and_smooth_scrolling_and_on_leaves_the_defaults() {
		let mut on = egui::Style::default();
		set_motion(&mut on, true);
		assert!(on.animation_time > 0.0, "egui's default fade stays");
		assert_eq!(on.scroll_animation, ScrollAnimation::default());
		let mut off = egui::Style::default();
		set_motion(&mut off, false);
		assert!(
			off.animation_time.abs() < f32::EPSILON,
			"{}",
			off.animation_time
		);
		assert_eq!(off.scroll_animation, ScrollAnimation::none());
	}
}
