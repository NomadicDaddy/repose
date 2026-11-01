//! Drawing one diff row: separators, side-by-side cells and inline lines, gutters, backgrounds,
//! word marks and syntax colour. Pure painting over a rect the pane has already allocated.

use std::ops::Range;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Rect, Ui, pos2};
use repose_core::model::{DiffLine, FileDiff, LineKind};

use crate::diff_rows::{LineRef, Row, line};
use crate::highlight::Span;
use crate::state::FileColours;
use crate::theme::*;

pub(super) const ROW_H: f32 = 20.0;
const GUTTER: f32 = 48.0;
const MONO: f32 = 12.0;

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
			let text = if heading.is_empty() {
				format!("⋯  {skipped} unchanged lines")
			} else {
				format!("⋯  {skipped} unchanged lines   {heading}")
			};
			p.text(
				pos2(rect.left() + 8.0, rect.center().y),
				Align2::LEFT_CENTER,
				text,
				FontId::new(TEXT_SMALL, FontFamily::Proportional),
				MUTED,
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

fn colours(kind: LineKind) -> (Color32, Color32) {
	match kind {
		LineKind::Added => (ADDED_BG, ADDED_MARK),
		LineKind::Removed => (REMOVED_BG, REMOVED_MARK),
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

fn gutter(p: &egui::Painter, left: f32, rect: Rect, no: Option<u32>) {
	if let Some(n) = no {
		p.text(
			pos2(left + GUTTER - 8.0, rect.center().y),
			Align2::RIGHT_CENTER,
			n.to_string(),
			FontId::new(MONO - 1.0, FontFamily::Monospace),
			MUTED,
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
		TEXT,
	);
}

/// One line as a layout job: runs cut at every mark and colour boundary, each with its syntax
/// colour and, inside a word mark, the mark background. A CR is shown as ␍ so a line-ending
/// change is visible. Marks and spans fall on character boundaries; any that do not are skipped.
fn marked_job(text: &str, marks: &[Range<usize>], mark_bg: Color32, spans: &[Span]) -> LayoutJob {
	let mut job = LayoutJob::default();
	job.wrap.max_width = f32::INFINITY;
	let font = FontId::new(MONO, FontFamily::Monospace);
	let mut cuts: Vec<usize> = vec![0, text.len()];
	cuts.extend(marks.iter().flat_map(|m| [m.start, m.end]));
	cuts.extend(spans.iter().flat_map(|s| [s.range.start, s.range.end]));
	cuts.retain(|&c| c <= text.len() && text.is_char_boundary(c));
	cuts.sort_unstable();
	cuts.dedup();
	for w in cuts.windows(2) {
		let (a, b) = (w[0], w[1]);
		if a >= b {
			continue;
		}
		let marked = marks.iter().any(|m| m.start <= a && b <= m.end);
		let color = spans
			.iter()
			.find(|s| s.range.start <= a && b <= s.range.end)
			.map_or(TEXT, |s| s.color);
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
