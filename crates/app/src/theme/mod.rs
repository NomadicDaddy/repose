//! The dark and light themes: tokens (dark from look.jpg) and the two fonts. Screens use these
//! names and never a colour literal.

pub mod contrast;
pub mod fonts;
pub mod tokens;

use eframe::egui;
use repose_core::config::Theme;

pub use tokens::*;

/// Fonts, both themes' visuals and spacing, and which theme to draw: `system` follows the Windows
/// app theme, which winit reports at start-up and whenever it changes (light-theme clause 2).
pub fn apply(ctx: &egui::Context, choice: Theme) {
	ctx.set_fonts(fonts::definitions());
	ctx.set_visuals_of(egui::Theme::Dark, visuals(false));
	ctx.set_visuals_of(egui::Theme::Light, visuals(true));
	ctx.set_theme(match choice {
		Theme::System => egui::ThemePreference::System,
		Theme::Dark => egui::ThemePreference::Dark,
		Theme::Light => egui::ThemePreference::Light,
	});
	ctx.all_styles_mut(|style| {
		style.spacing.item_spacing = egui::vec2(8.0, 6.0);
		style.spacing.button_padding = egui::vec2(10.0, 5.0);
	});
}

/// At the start of every frame: the tokens this thread paints with follow the theme egui is
/// drawing now, so a switch takes effect on the next frame with no state reset.
pub fn begin_frame(ctx: &egui::Context) {
	set_light(ctx.theme() == egui::Theme::Light);
}

/// egui's own widgets in one theme's tokens.
fn visuals(light: bool) -> egui::Visuals {
	let pick = |c: Colour| c.of(light);
	let stroke = |c: Colour| egui::Stroke::new(1.0, pick(c));
	let mut v = if light {
		egui::Visuals::light()
	} else {
		egui::Visuals::dark()
	};
	v.panel_fill = pick(BG_APP);
	v.window_fill = pick(BG_PANEL);
	v.extreme_bg_color = pick(BG_APP);
	v.faint_bg_color = pick(BG_RAISED);
	v.override_text_color = Some(pick(TEXT));
	v.selection.bg_fill = pick(ACCENT_BG);
	v.selection.stroke = stroke(ACCENT_LINE);
	v.hyperlink_color = pick(ACCENT);
	v.widgets.noninteractive.bg_stroke = stroke(LINE_SOFT);
	v.widgets.noninteractive.bg_fill = pick(BG_PANEL);
	v.widgets.inactive.bg_fill = pick(BG_RAISED);
	v.widgets.inactive.weak_bg_fill = pick(BG_RAISED);
	v.widgets.inactive.bg_stroke = stroke(LINE);
	v.widgets.hovered.bg_fill = pick(BG_HOVER);
	v.widgets.hovered.weak_bg_fill = pick(BG_HOVER);
	v.widgets.hovered.bg_stroke = stroke(LINE_STRONG);
	v.widgets.active.bg_fill = pick(ACCENT_BG);
	v.widgets.active.weak_bg_fill = pick(ACCENT_BG);
	v.widgets.active.bg_stroke = stroke(ACCENT_LINE);
	v
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
