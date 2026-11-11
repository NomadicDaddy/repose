//! The Activity screen (activity-screen): four tiles for what needs attention, then a grid of each
//! repository's local commits per day over the last 30 days (`activity_grid`). It reads only what
//! repose already reads offline. A tile filters the grid to its repositories; activating it again,
//! or Clear, shows them all.

use std::cmp::Reverse;

use eframe::egui::{self, RichText, Role, Sense, Stroke, Ui};
use repose_core::model::RepoSummary;

use super::a11y;
use super::activity_grid::{grid, legend, quiet_row};
use crate::state::AppState;
use crate::state::activity::{
	ActivityRead, DAYS, Tile, day_counts, in_tile, local_today, now_secs,
};
use crate::theme::*;

/// A tile: which repositories it counts, its label, the note under it, and its number's colour.
struct TileSpec {
	tile: Tile,
	label: &'static str,
	note: &'static str,
	colour: Colour,
}

const TILES: [TileSpec; 4] = [
	TileSpec {
		tile: Tile::Uncommitted,
		label: "Uncommitted changes",
		note: "Changed or staged files",
		colour: WARN,
	},
	TileSpec {
		tile: Tile::Ahead,
		label: "Ahead of upstream",
		note: "From saved refs; no fetch",
		colour: ACCENT,
	},
	TileSpec {
		tile: Tile::Behind,
		label: "Behind upstream",
		note: "From saved refs; no fetch",
		colour: INFO,
	},
	TileSpec {
		tile: Tile::Quiet,
		label: "Quiet for 60+ days",
		note: "No local commit in 60 days",
		colour: MUTED,
	},
];

/// One grid row: a repository and its commits per day, or None while its read is pending or
/// failed; `truncated` when the read stopped at its bound, so the counts may be low.
pub(super) struct GridRow {
	pub(super) repo: RepoSummary,
	pub(super) counts: Option<[u32; DAYS]>,
	pub(super) failed: bool,
	pub(super) truncated: bool,
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
	let now = now_secs();
	let offset = state.utc_offset.unwrap_or(0);
	let title = RichText::new("Activity across all repositories");
	ui.label(title.size(TEXT_DETAIL_TITLE).color(TEXT_BRIGHT));
	let count = format!("{} repositories · Saved refs; no fetch", state.repos.len());
	ui.label(RichText::new(count).size(TEXT_META).color(MUTED));
	ui.add_space(12.0);
	tiles(ui, state, now);
	if let Some(tile) = state.activity.filter {
		ui.horizontal(|ui| {
			let name = TILES
				.iter()
				.find(|t| t.tile == tile)
				.map_or("", |t| t.label);
			ui.label(RichText::new(format!("Showing: {name}")).color(MUTED));
			if ui.button("Clear").clicked() {
				state.activity.filter = None;
			}
		});
	}
	ui.add_space(12.0);
	let today = local_today(now, offset);
	let (mut drawn, quiet) = rows(state, now, today);
	// Under the Quiet tile every row is quiet, so nothing folds; otherwise the closing row opens
	// the quiet repositories into the grid as rows like any other (Roger 5Q-1 on 4051085).
	let folds = state.activity.filter != Some(Tile::Quiet);
	let quiet_count = quiet.len();
	if !folds || state.activity.quiet_open {
		drawn.extend(quiet);
	}
	egui::ScrollArea::vertical()
		.auto_shrink(false)
		.show(ui, |ui| {
			grid(ui, state, &drawn, today);
			if folds && quiet_count > 0 {
				quiet_row(ui, state, quiet_count);
			}
			legend(ui);
		});
}

fn tiles(ui: &mut Ui, state: &mut AppState, now: i64) {
	let reads = &state.activity.reads;
	let counts: Vec<usize> = TILES
		.iter()
		.map(|spec| {
			let repos = state.repos.iter();
			repos
				.filter(|r| in_tile(spec.tile, r, reads.get(&r.path), now))
				.count()
		})
		.collect();
	ui.columns(TILES.len(), |cols| {
		for (i, (spec, col)) in TILES.iter().zip(cols.iter_mut()).enumerate() {
			let TileSpec {
				tile,
				label,
				note,
				colour,
			} = spec;
			let on = state.activity.filter == Some(*tile);
			let line = if on { ACCENT_LINE.get() } else { LINE.get() };
			let frame = egui::Frame::new()
				.fill(if on { ACCENT_BG.get() } else { BG_PANEL.get() })
				.stroke(Stroke::new(1.0, line))
				.corner_radius(RADIUS)
				.inner_margin(12.0);
			let inner = frame.show(col, |ui| {
				ui.set_min_width(ui.available_width());
				let number = RichText::new(counts[i].to_string());
				ui.label(number.size(28.0).color(*colour));
				ui.label(RichText::new(*label).color(TEXT_BRIGHT).strong());
				ui.label(RichText::new(*note).size(TEXT_SMALL).color(MUTED));
			});
			let id = col.scope_id().with(("activity-tile", i));
			let response = col.interact(inner.response.rect, id, Sense::click());
			let name = format!("{label}: {}. {note}", counts[i]);
			a11y::control(&response, Role::Button, Some(on), &name);
			if response.clicked() {
				state.activity.filter = if on { None } else { Some(*tile) };
				state.activity.cell = (0, Some(DAYS - 1));
			}
		}
	});
}

/// The rows to draw, filtered by the tile, newest activity first; and the quiet ones (read, with
/// no commit in the grid's days) apart, for the folded closing row.
fn rows(state: &AppState, now: i64, today: i64) -> (Vec<GridRow>, Vec<GridRow>) {
	let reads = &state.activity.reads;
	let filter = state.activity.filter;
	let mut all: Vec<(Option<i64>, GridRow)> = state
		.repos
		.iter()
		.filter(|r| filter.is_none_or(|t| in_tile(t, r, reads.get(&r.path), now)))
		.map(|r| {
			let read = reads.get(&r.path);
			let (newest, counts) = match read {
				Some(ActivityRead::Read(a)) => (a.newest, Some(day_counts(&a.commits, today))),
				_ => (None, None),
			};
			let failed = matches!(read, Some(ActivityRead::Failed(_)));
			let truncated = matches!(read, Some(ActivityRead::Read(a)) if a.truncated);
			(
				newest,
				GridRow {
					repo: r.clone(),
					counts,
					failed,
					truncated,
				},
			)
		})
		.collect();
	all.sort_by_key(|(newest, _)| Reverse(*newest));
	let (quiet, active): (Vec<_>, Vec<_>) = all
		.into_iter()
		.map(|(_, row)| row)
		.partition(|r| r.counts.is_some_and(|c| c.iter().all(|n| *n == 0)));
	(active, quiet)
}
