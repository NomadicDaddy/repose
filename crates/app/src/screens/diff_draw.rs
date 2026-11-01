//! Drawing one diff row: separators, side-by-side cells and inline lines, gutters, backgrounds
//! and the word marks. Pure painting over a rect the pane has already allocated.

use std::ops::Range;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Rect, Ui, pos2};
use repose_core::model::{DiffLine, FileDiff, LineKind};

use crate::diff_rows::{Row, line};
use crate::theme::*;

pub(super) const ROW_H: f32 = 20.0;
const GUTTER: f32 = 48.0;
const MONO: f32 = 12.0;

pub(super) fn draw_row(ui: &Ui, rect: Rect, file: &FileDiff, row: &Row) {
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
			cell(ui, left, old.map(|r| line(file, r)), old_marks, true);
			cell(ui, right, new.map(|r| line(file, r)), new_marks, false);
			p.line_segment(
				[pos2(mid, rect.top()), pos2(mid, rect.bottom())],
				egui::Stroke::new(1.0, LINE),
			);
		}
		Row::Line { line: r, marks } => {
			let l = line(file, *r);
			let (bg, mark) = colours(l.kind);
			p.rect_filled(rect, 0.0, bg);
			gutter(&p, rect.left(), rect, l.old_no);
			gutter(&p, rect.left() + GUTTER, rect, l.new_no);
			let text_rect =
				Rect::from_min_max(pos2(rect.left() + 2.0 * GUTTER, rect.top()), rect.max);
			text_cell(ui, text_rect, &l.text, marks, mark);
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

fn cell(ui: &Ui, rect: Rect, l: Option<&DiffLine>, marks: &[Range<usize>], old_side: bool) {
	let p = ui.painter_at(rect);
	let Some(l) = l else {
		p.rect_filled(rect, 0.0, BG_PANEL);
		return;
	};
	let (bg, mark) = colours(l.kind);
	p.rect_filled(rect, 0.0, bg);
	gutter(
		&p,
		rect.left(),
		rect,
		if old_side { l.old_no } else { l.new_no },
	);
	let text_rect = Rect::from_min_max(pos2(rect.left() + GUTTER, rect.top()), rect.max);
	text_cell(ui, text_rect, &l.text, marks, mark);
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

fn text_cell(ui: &Ui, rect: Rect, text: &str, marks: &[Range<usize>], mark_bg: Color32) {
	let job = marked_job(text, marks, mark_bg);
	let galley = ui.fonts_mut(|f| f.layout_job(job));
	let p = ui.painter_at(rect);
	p.galley(
		pos2(rect.left() + 6.0, rect.center().y - galley.size().y / 2.0),
		galley,
		TEXT,
	);
}

/// One line as a layout job: plain runs and marked runs, a CR shown as ␍ so a line-ending change
/// is visible. Marks come from word boundaries and so fall on character boundaries.
fn marked_job(text: &str, marks: &[Range<usize>], mark_bg: Color32) -> LayoutJob {
	let mut job = LayoutJob::default();
	job.wrap.max_width = f32::INFINITY;
	let font = FontId::new(MONO, FontFamily::Monospace);
	let mut append = |seg: &str, bg: Color32| {
		if seg.is_empty() {
			return;
		}
		let shown = seg.replace('\r', "␍");
		job.append(
			&shown,
			0.0,
			TextFormat {
				font_id: font.clone(),
				color: TEXT,
				background: bg,
				..Default::default()
			},
		);
	};
	let mut pos = 0;
	for m in marks {
		if m.start > pos && text.is_char_boundary(m.start) {
			append(&text[pos..m.start], Color32::TRANSPARENT);
			pos = m.start;
		}
		if m.end > pos && text.is_char_boundary(m.end) {
			append(&text[pos..m.end], mark_bg);
			pos = m.end;
		}
	}
	if pos < text.len() {
		append(&text[pos..], Color32::TRANSPARENT);
	}
	job
}
