//! One syntax theme's colours for the highlighter (crate::highlight): a scope stack's
//! foreground, lifted to AA on that theme's own span backgrounds, or None where it is the theme's
//! plain text.

use std::collections::HashMap;

use eframe::egui::Color32;
use syntect::highlighting::{Highlighter, Theme};
use syntect::parsing::ScopeStack;

use crate::theme::contrast::readable;

/// One theme's colours for a scope stack. The theme's colours are lifted to AA on its own span
/// backgrounds (theme/contrast.rs); a theme has a handful of colours, so each is worked out once
/// per call, not once per span.
pub(crate) struct Palette<'t> {
	highlighter: Highlighter<'t>,
	plain: Option<syntect::highlighting::Color>,
	light: bool,
	pub(crate) text: Color32,
	fixed: HashMap<(u8, u8, u8), Color32>,
}

impl<'t> Palette<'t> {
	pub(crate) fn new(theme: &'t Theme, light: bool) -> Self {
		Self {
			highlighter: Highlighter::new(theme),
			plain: theme.settings.foreground,
			light,
			text: crate::theme::TEXT.of(light),
			fixed: HashMap::new(),
		}
	}

	/// The readable colour for text under `stack`, or None where it is the plain foreground.
	pub(crate) fn colour(&mut self, stack: &ScopeStack) -> Option<Color32> {
		let c = self
			.highlighter
			.style_for_stack(stack.as_slice())
			.foreground;
		if Some(c) == self.plain {
			return None;
		}
		let light = self.light;
		Some(
			*self
				.fixed
				.entry((c.r, c.g, c.b))
				.or_insert_with(|| readable(Color32::from_rgb(c.r, c.g, c.b), light)),
		)
	}
}
