//! Activity's loading glyphs (activity-loading-glyphs). While any row is still reading, every day
//! cell of a row that has not failed shows one glyph, digits and half-width katakana in the accent
//! ramp's steps, each on its own 80 to 200 ms cycle, with a brighter head falling down each day's
//! column. Rows that finish early keep their glyphs; once the last row arrives, all of them
//! cross-fade into their real fills together over `FADE` seconds, and the grid is then exactly the
//! plain grid for the rest of the session. With animations off, and until Windows' setting is
//! known, the glyphs stand still and the swap is instant (Roger 5Q-2 on 23fdba0); after `CAP`
//! seconds of falling, a read that never reports no longer keeps the grid animating (Morticia,
//! note 1 on 23fdba0). Repaints are asked for only while glyphs fall or fade.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::time::Duration;

use eframe::egui::{Color32, FontFamily, FontId, Painter, Rect, Stroke, StrokeKind, Ui};

use super::activity_grid::step_colour;
use crate::state::AppState;
use crate::state::activity::{ActivityRead, GlyphPhase};
use crate::theme::fonts::{GLYPH_FAMILY, add_glyph_font};
use crate::theme::{motion_known, *};

/// The cross-fade, in seconds (clause 2: 300 to 400 ms).
pub(super) const FADE: f64 = 0.35;
const DIGITS: &str = "0123456789";
const KANA: &str = "ｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜﾝ";
/// How fast a column's bright head falls, in rows a second, and the rows its trail spans.
const FALL: f64 = 9.0;
const TAIL: f64 = 4.0;
/// Rows in one fall, so a column's head comes round again even on a short grid.
const CYCLE: f64 = 14.0;
/// The repaint interval while the glyphs fall: well under the shortest glyph cycle.
const TICK: Duration = Duration::from_millis(40);
/// Seconds of falling after which the glyphs stand still.
pub(super) const CAP: f64 = 30.0;

/// The next phase, from whether any repository is listed, whether any is still reading, the
/// frame's time and whether the glyphs may move.
pub(super) fn next(
	phase: GlyphPhase,
	listed: bool,
	reading: bool,
	now: f64,
	animate: bool,
) -> GlyphPhase {
	match phase {
		GlyphPhase::Waiting if !listed => GlyphPhase::Waiting,
		GlyphPhase::Waiting if reading => GlyphPhase::Falling(now),
		GlyphPhase::Falling(_) if reading => phase,
		GlyphPhase::Falling(_) if animate => GlyphPhase::Fading(now),
		GlyphPhase::Fading(from) if animate && now - from < FADE => phase,
		_ => GlyphPhase::Done,
	}
}

/// What one frame's glyphs share.
pub(super) struct Glyphs {
	time: f64,
	/// How far the fade has gone, 0 while falling.
	fade: f32,
	family: FontFamily,
	set: Vec<char>,
}

/// This frame's glyphs, or None once they are done (or before any repository is listed).
pub(super) fn begin(ui: &Ui, state: &mut AppState) -> Option<Glyphs> {
	let reads = &state.activity.reads;
	let reading = state.repos.iter().any(|r| {
		let read = reads.get(&r.path);
		matches!(read, None | Some(ActivityRead::Loading))
	});
	let ctx = ui.ctx();
	let now = ui.input(|i| i.time);
	let capped = matches!(state.activity.glyphs, GlyphPhase::Falling(t) if now - t >= CAP);
	let animate = motion_known(ctx) && ui.style().animation_time > 0.0 && !capped;
	let listed = !state.repos.is_empty();
	let phase = next(state.activity.glyphs, listed, reading, now, animate);
	state.activity.glyphs = phase;
	let fade = match phase {
		GlyphPhase::Falling(_) => 0.0,
		GlyphPhase::Fading(from) => ((now - from) / FADE).clamp(0.0, 1.0) as f32,
		GlyphPhase::Waiting | GlyphPhase::Done => return None,
	};
	if !state.activity.glyph_font_asked {
		state.activity.glyph_font_asked = true;
		add_glyph_font(ctx);
	}
	if animate {
		match phase {
			GlyphPhase::Fading(_) => ctx.request_repaint(),
			_ => ctx.request_repaint_after(TICK),
		}
	}
	// The katakana join once their font is in; until then, or without it, digits alone.
	let named = FontFamily::Name(GLYPH_FAMILY.into());
	let (family, set) = if ctx.fonts(|f| f.families().contains(&named)) {
		(named, format!("{DIGITS}{KANA}"))
	} else {
		(FontFamily::Monospace, DIGITS.to_string())
	};
	Some(Glyphs {
		time: if animate { now } else { 0.0 },
		fade,
		family,
		set: set.chars().collect(),
	})
}

/// A stable number for a repository, so its glyphs keep their rhythm when the rows re-sort.
pub(super) fn seed(path: &Path) -> u64 {
	let mut h = DefaultHasher::new();
	path.hash(&mut h);
	h.finish()
}

/// SplitMix64: a well-spread number from two.
fn mix(a: u64, b: u64) -> u64 {
	let mut z = a ^ b.wrapping_mul(0x9E37_79B9_7F4A_7C15);
	z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
	z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
	z ^ (z >> 31)
}

/// One day cell at `rect`. Without glyphs it is the plain box in `fill`; with them its fill
/// cross-fades from the panel colour to `fill` and its glyph fades out. `at` is the row's
/// repository seed, its place in the grid, and the day's column.
pub(super) fn paint_cell(
	painter: &Painter,
	rect: Rect,
	glyphs: Option<&Glyphs>,
	at: (u64, usize, usize),
	fill: Color32,
) {
	let shown = glyphs.map_or(fill, |g| BG_PANEL.get().lerp_to_gamma(fill, g.fade));
	let outline = Stroke::new(1.0, LINE_SOFT.get());
	painter.rect(rect, RADIUS, shown, outline, StrokeKind::Inside);
	if let Some(g) = glyphs {
		g.glyph(painter, rect, at);
	}
}

impl Glyphs {
	fn glyph(&self, painter: &Painter, rect: Rect, (seed, row, day): (u64, usize, usize)) {
		let own = mix(seed, day as u64);
		let period = 80.0 + (own % 121) as f64;
		let offset = (own >> 16) as f64 % period;
		let tick = ((self.time * 1000.0 + offset) / period).floor() as u64;
		let glyph = self.set[(mix(own, tick) % self.set.len() as u64) as usize];
		// Each day's column has its own head, falling down the rows with a trail behind it.
		let lane = (mix(day as u64, 0x5EED) % 1000) as f64 / 1000.0 * CYCLE;
		let head = (self.time * FALL + lane).rem_euclid(CYCLE);
		let behind = (head - row as f64).rem_euclid(CYCLE);
		let lit = match behind {
			b if b < 1.0 => 4,
			b if b < TAIL => 3,
			_ => 2,
		};
		let colour = step_colour(lit).gamma_multiply(1.0 - self.fade);
		let size = (rect.height().min(rect.width() * 1.6) * 0.62).min(16.0);
		let font = FontId::new(size, self.family.clone());
		// Laid out once per glyph and size, then tinted with an override colour: a placeholder's
		// fallback colour painted white natively (the f1e55af captures).
		let galley = painter.layout_no_wrap(glyph.to_string(), font, Color32::PLACEHOLDER);
		let at = rect.center() - galley.size() / 2.0;
		painter.galley_with_override_text_color(at, galley, colour);
	}
}

#[cfg(test)]
mod tests {
	use super::{FADE, next};
	use crate::state::activity::GlyphPhase::{Done, Fading, Falling, Waiting};

	/// Clauses 2 and 3: glyphs fall only while a row reads, fade from the moment the last row
	/// lands, end after the fade, and never come back; with animations off there is no fade.
	#[test]
	fn the_phases_follow_the_reads() {
		assert_eq!(
			next(Waiting, false, false, 0.0, true),
			Waiting,
			"nothing listed yet"
		);
		assert_eq!(next(Waiting, true, true, 0.5, true), Falling(0.5));
		assert_eq!(next(Waiting, true, false, 0.0, true), Done, "already read");
		assert_eq!(next(Falling(0.5), true, true, 1.0, true), Falling(0.5));
		assert_eq!(next(Falling(0.5), true, false, 2.0, true), Fading(2.0));
		assert_eq!(next(Fading(2.0), true, false, 2.2, true), Fading(2.0));
		assert_eq!(next(Fading(2.0), true, false, 2.0 + FADE, true), Done);
		assert_eq!(next(Done, true, true, 9.0, true), Done, "a later refresh");
		assert_eq!(
			next(Falling(0.5), true, false, 2.0, false),
			Done,
			"animations off or not yet known"
		);
	}
}
