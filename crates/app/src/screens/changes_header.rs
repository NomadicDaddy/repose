//! The Changes group header's painted pieces: the fold triangle and the file-count pill.

use eframe::egui::{self, FontFamily, FontId, Sense, StrokeKind, Ui, pos2, vec2};

use crate::screens::a11y;
use crate::theme::*;

/// A small triangle at `centre`, pointing down when the group is open and right when folded,
/// painted rather than a glyph (the ▸ glyph sat small and above the baseline, design-polish
/// clause 5).
pub(super) fn disclosure(painter: &egui::Painter, centre: egui::Pos2, open: bool) {
	let half = TEXT_SMALL * 0.3;
	let points = if open {
		vec![
			pos2(centre.x - half, centre.y - half * 0.6),
			pos2(centre.x + half, centre.y - half * 0.6),
			pos2(centre.x, centre.y + half * 0.6),
		]
	} else {
		vec![
			pos2(centre.x - half * 0.6, centre.y - half),
			pos2(centre.x - half * 0.6, centre.y + half),
			pos2(centre.x + half * 0.6, centre.y),
		]
	};
	painter.add(egui::Shape::convex_polygon(
		points,
		MUTED.get(),
		egui::Stroke::NONE,
	));
}

pub(super) fn count_pill(ui: &mut Ui, n: usize) {
	let font = FontId::new(TEXT_SMALL, FontFamily::Proportional);
	let galley = ui
		.painter()
		.layout_no_wrap(n.to_string(), font, ACCENT.get());
	let size = galley.size() + vec2(12.0, 4.0);
	let (rect, pill) = ui.allocate_exact_size(size, Sense::hover());
	let name = crate::screens::count_label(n, "file", "files");
	a11y::text(ui, pill.id, rect, egui::Role::Label, &name);
	ui.painter().rect(
		rect,
		9.0,
		ACCENT_BG,
		egui::Stroke::new(1.0, ACCENT_LINE),
		StrokeKind::Inside,
	);
	ui.painter().galley(
		pos2(rect.left() + 6.0, rect.top() + 2.0),
		galley,
		ACCENT.get(),
	);
}
