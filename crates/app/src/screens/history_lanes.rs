//! Drawing History's branch lanes (branch-graph-lanes clause 2): each row's lane segments at the
//! row's left, and the ref names on its commit as pills before the meta text. A straight lane is
//! muted; a connector, where a lane merges into a commit or a commit forks a lane, is accent; a
//! merge commit's dot is accent and any other is muted.

use eframe::egui::{Painter, Pos2, Rect, Stroke, pos2};
use repose_core::model::Commit;

use crate::lanes::GraphRow;
use crate::screens::row::pill;
use crate::theme::*;

/// One column's width.
pub const LANE_W: f32 = 14.0;
const LEFT: f32 = 8.0;

/// The width the graph takes at the left of every row: the widest row loaded, so the subjects
/// line up down the list. Zero when there is no graph to draw.
pub fn graph_width(graph: &[GraphRow]) -> f32 {
	match graph.iter().map(|r| r.width).max() {
		Some(w) if w > 0 => LEFT + w as f32 * LANE_W,
		_ => 0.0,
	}
}

/// Paints `row` into the row `rect`. The segments run past the row's edges by `gap / 2`, the
/// half of the spacing between rows, so a lane meets the next row's lane with no break.
pub fn draw(painter: &Painter, rect: Rect, gap: f32, row: &GraphRow, merge: bool) {
	let x = |col: usize| rect.left() + LEFT + col as f32 * LANE_W + LANE_W / 2.0;
	let (top, mid, bottom) = (
		rect.top() - gap / 2.0,
		rect.center().y,
		rect.bottom() + gap / 2.0,
	);
	let segment = |a: Pos2, b: Pos2, connector: bool| {
		let colour = if connector { ACCENT.get() } else { MUTED.get() };
		painter.line_segment([a, b], Stroke::new(1.5, colour));
	};
	for &(from, to) in &row.up {
		segment(pos2(x(from), top), pos2(x(to), mid), from != to);
	}
	for &(from, to) in &row.down {
		segment(pos2(x(from), mid), pos2(x(to), bottom), from != to);
	}
	let dot = if merge { ACCENT.get() } else { MUTED.get() };
	painter.circle_filled(pos2(x(row.node), mid), 4.0, dot);
}

/// The commit's ref names as pills ending at `right`, nearest first; returns where the pills
/// begin, so the subject stops short of them. Tags are info-coloured, branches accent.
pub fn refs(painter: &Painter, c: &Commit, right: f32, cy: f32) -> f32 {
	let mut right = right;
	for name in c.refs.iter().rev() {
		let colour = if name.starts_with("tag: ") {
			INFO
		} else {
			ACCENT
		};
		right = pill(painter, right, cy, name, colour) - 4.0;
	}
	right
}
