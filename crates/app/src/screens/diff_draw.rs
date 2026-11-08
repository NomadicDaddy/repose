//! Drawing one diff row: separators, side-by-side cells and inline lines, gutters, backgrounds,
//! word marks and syntax colour. Pure painting over a rect the pane has already allocated.

use std::ops::Range;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Rect, Role, Ui, pos2};

use super::a11y;
use repose_core::model::{DiffLine, FileDiff, LineKind};

use crate::diff_rows::{LineRef, Row, line};
use crate::highlight::Span;
use crate::state::FileColours;
use crate::theme::*;

pub(super) const GUTTER: f32 = 48.0;
/// The advance of one glyph at `TEXT_MONO`, for sizing a row to its longest line; an estimate, since
/// the galley is laid out only for visible rows.
pub(super) const MONO_ADVANCE: f32 = 7.4;

/// One file's parsed lines and their colours, as the rows refer to them.
pub(super) struct Source<'a> {
	pub file: &'a FileDiff,
	pub colours: Option<&'a FileColours>,
}

impl Source<'_> {
	fn spans(&self, r: LineRef) -> &[Span] {
		self.colours
			.and_then(|c| c.get(r.hunk))
			.and_then(|h| h.get(r.line))
			.map_or(&[], Vec::as_slice)
	}
}

pub(super) fn draw_row(ui: &Ui, rect: Rect, src: &Source<'_>, row: &Row) {
	let p = ui.painter_at(rect);
	match row {
		Row::Separator { skipped, heading } => {
			p.rect_filled(rect, 0.0, BG_RAISED);
			let lines = super::count_label(*skipped as usize, "unchanged line", "unchanged lines");
			let text = if heading.is_empty() {
				format!("⋯  {lines}")
			} else {
				format!("⋯  {lines}   {heading}")
			};
			p.text(
				pos2(rect.left() + 8.0, rect.center().y),
				Align2::LEFT_CENTER,
				text,
				FontId::new(TEXT_SMALL, FontFamily::Proportional),
				MUTED.get(),
			);
		}
		Row::Pair {
			old,
			new,
			old_marks,
			new_marks,
		} => {
			let mid = rect.left() + rect.width() / 2.0;
			let left = Rect::from_min_max(rect.min, pos2(mid, rect.bottom()));
			let right = Rect::from_min_max(pos2(mid, rect.top()), rect.max);
			cell(ui, left, src, *old, old_marks, true);
			cell(ui, right, src, *new, new_marks, false);
			p.line_segment(
				[pos2(mid, rect.top()), pos2(mid, rect.bottom())],
				egui::Stroke::new(1.0, LINE),
			);
		}
		Row::Line { line: r, marks } => {
			let l = line(src.file, *r);
			let (bg, mark) = colours(l.kind);
			p.rect_filled(rect, 0.0, bg);
			gutter(&p, rect.left(), rect, l.old_no);
			gutter(&p, rect.left() + GUTTER, rect, l.new_no);
			let text_rect =
				Rect::from_min_max(pos2(rect.left() + 2.0 * GUTTER, rect.top()), rect.max);
			text_cell(ui, text_rect, &l.text, marks, mark, src.spans(*r));
		}
	}
}

/// The row's text node for assistive technology: a separator as a heading, a line or a
/// side-by-side pair as its numbers, kind and text. Never focusable, so the diff adds no Tab
/// stops (accessible-rendered-content clause 3).
pub(super) fn describe_row(ui: &Ui, id: egui::Id, rect: Rect, src: &Source<'_>, row: &Row) {
	let (role, text) = match row {
		Row::Separator { skipped, heading } => (Role::Heading, a11y::separator(*skipped, heading)),
		Row::Pair { old, new, .. } => {
			let sides: Vec<String> = [*old, *new]
				.into_iter()
				.flatten()
				.map(|r| a11y::diff_line(line(src.file, r)))
				.collect();
			(Role::Label, sides.join("; "))
		}
		Row::Line { line: r, .. } => (Role::Label, a11y::diff_line(line(src.file, *r))),
	};
	a11y::text(ui, id, rect, role, &text);
}

fn colours(kind: LineKind) -> (Color32, Color32) {
	match kind {
		LineKind::Added => (ADDED_BG.get(), ADDED_MARK.get()),
		LineKind::Removed => (REMOVED_BG.get(), REMOVED_MARK.get()),
		LineKind::Context => (Color32::TRANSPARENT, Color32::TRANSPARENT),
	}
}

fn cell(
	ui: &Ui,
	rect: Rect,
	src: &Source<'_>,
	r: Option<LineRef>,
	marks: &[Range<usize>],
	old_side: bool,
) {
	let p = ui.painter_at(rect);
	let Some(r) = r else {
		p.rect_filled(rect, 0.0, BG_PANEL);
		return;
	};
	let l: &DiffLine = line(src.file, r);
	let (bg, mark) = colours(l.kind);
	p.rect_filled(rect, 0.0, bg);
	gutter(
		&p,
		rect.left(),
		rect,
		if old_side { l.old_no } else { l.new_no },
	);
	let text_rect = Rect::from_min_max(pos2(rect.left() + GUTTER, rect.top()), rect.max);
	text_cell(ui, text_rect, &l.text, marks, mark, src.spans(r));
}

/// How a file-view line looks besides its text: the current line, and the line holding keyboard
/// focus.
pub(super) struct LineLook {
	pub current: bool,
	pub focused: bool,
}

/// One line of a file in the read-only viewer: a single gutter and the coloured text, drawn with
/// the same font, row height and colours as a diff line; the current line sits on the accent
/// fill, and the focused line has the one-pixel accent outline every list row uses
/// (keyboard-row-focus), over the fill when it is also the current line.
pub(super) fn draw_file_line(
	ui: &Ui,
	rect: Rect,
	no: u32,
	text: &str,
	spans: &[Span],
	look: LineLook,
) {
	let p = ui.painter_at(rect);
	let fill = look.current.then_some(ACCENT_BG);
	super::row::frame(&p, rect, 0.0, fill, None, look.focused);
	gutter(&p, rect.left(), rect, Some(no));
	let text_rect = Rect::from_min_max(pos2(rect.left() + GUTTER, rect.top()), rect.max);
	text_cell(ui, text_rect, text, &[], Color32::TRANSPARENT, spans);
}

fn gutter(p: &egui::Painter, left: f32, rect: Rect, no: Option<u32>) {
	if let Some(n) = no {
		p.text(
			pos2(left + GUTTER - 8.0, rect.center().y),
			Align2::RIGHT_CENTER,
			n.to_string(),
			FontId::new(TEXT_MONO - 1.0, FontFamily::Monospace),
			MUTED.get(),
		);
	}
}

fn text_cell(
	ui: &Ui,
	rect: Rect,
	text: &str,
	marks: &[Range<usize>],
	mark_bg: Color32,
	spans: &[Span],
) {
	let job = marked_job(text, marks, mark_bg, spans);
	let galley = ui.fonts_mut(|f| f.layout_job(job));
	let p = ui.painter_at(rect);
	p.galley(
		pos2(rect.left() + 6.0, rect.center().y - galley.size().y / 2.0),
		galley,
		TEXT.get(),
	);
}

/// One line as a layout job: runs cut at every mark and colour boundary, each with its syntax
/// colour and, inside a word mark, the mark background. A CR is shown as ␍ so a line-ending
/// change is visible. Marks and spans are each sorted and non-overlapping (similar and syntect
/// both produce them in order), so one pass with a cursor into each finds every segment's
/// colour: linear in the number of boundaries, never a scan per segment (Roger on e8e10ad).
/// Boundaries off a character edge are skipped.
fn marked_job(text: &str, marks: &[Range<usize>], mark_bg: Color32, spans: &[Span]) -> LayoutJob {
	let mut job = LayoutJob::default();
	job.wrap.max_width = f32::INFINITY;
	let font = FontId::new(TEXT_MONO, FontFamily::Monospace);
	let mut cuts: Vec<usize> = Vec::with_capacity(2 + 2 * (marks.len() + spans.len()));
	cuts.push(0);
	cuts.push(text.len());
	cuts.extend(marks.iter().flat_map(|m| [m.start, m.end]));
	cuts.extend(spans.iter().flat_map(|s| [s.range.start, s.range.end]));
	cuts.retain(|&c| c <= text.len() && text.is_char_boundary(c));
	cuts.sort_unstable();
	cuts.dedup();
	let (mut mi, mut si) = (0usize, 0usize);
	for w in cuts.windows(2) {
		let (a, b) = (w[0], w[1]);
		if a >= b {
			continue;
		}
		while mi < marks.len() && marks[mi].end <= a {
			mi += 1;
		}
		while si < spans.len() && spans[si].range.end <= a {
			si += 1;
		}
		let marked = marks.get(mi).is_some_and(|m| m.start <= a && b <= m.end);
		let color = spans
			.get(si)
			.filter(|s| s.range.start <= a && b <= s.range.end)
			.map_or(TEXT.get(), |s| s.color.get());
		let shown = text[a..b].replace('\r', "␍");
		job.append(
			&shown,
			0.0,
			TextFormat {
				font_id: font.clone(),
				color,
				background: if marked {
					mark_bg
				} else {
					Color32::TRANSPARENT
				},
				..Default::default()
			},
		);
	}
	job
}

#[cfg(test)]
mod tests {
	use std::time::{Duration, Instant};

	use eframe::egui::Color32;

	use super::marked_job;
	use crate::highlight::Span;

	/// Roger on e8e10ad: the segment-to-colour lookup must not scan the spans per segment.
	#[test]
	fn a_span_rich_line_lays_out_in_linear_time() {
		let text: String = (0..1000).map(|i| format!("w{i} ")).collect();
		let mut spans = Vec::new();
		let mut pos = 0;
		for i in 0..1000 {
			let word = format!("w{i} ");
			spans.push(Span {
				range: pos..pos + 2,
				color: crate::theme::INFO,
			});
			pos += word.len();
		}
		let marks: Vec<std::ops::Range<usize>> = (0..500).map(|i| i * 8..i * 8 + 3).collect();
		let started = Instant::now();
		let job = marked_job(&text, &marks, Color32::RED, &spans);
		assert!(job.sections.len() >= 1000, "{}", job.sections.len());
		assert!(
			started.elapsed() < Duration::from_millis(50),
			"{:?}",
			started.elapsed()
		);
		assert_eq!(job.text.len(), text.len());
	}
}
