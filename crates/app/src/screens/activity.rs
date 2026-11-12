//! The Activity screen (activity-screen): an "All repositories" card and four tiles for what needs
//! attention, then a grid of each repository's local commits per day over the last 30 days
//! (`activity_grid`). It reads only what repose already reads offline. A tile filters the grid to
//! its repositories; activating it again, or the All card, shows them all
//! (activity-all-repositories-card).

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

/// A card: the filter it selects (None for every repository), its label, the note under it, and
/// its number's colour.
struct TileSpec {
	tile: Option<Tile>,
	label: &'static str,
	note: &'static str,
	colour: Colour,
}

const TILES: [TileSpec; 5] = [
	TileSpec {
		tile: None,
		label: "All repositories",
		note: "Every listed repository",
		colour: TEXT_BRIGHT,
	},
	TileSpec {
		tile: Some(Tile::Uncommitted),
		label: "Uncommitted changes",
		note: "Changed or staged files",
		colour: WARN,
	},
	TileSpec {
		tile: Some(Tile::Ahead),
		label: "Ahead of upstream",
		note: "From saved refs; no fetch",
		colour: ACCENT,
	},
	TileSpec {
		tile: Some(Tile::Behind),
		label: "Behind upstream",
		note: "From saved refs; no fetch",
		colour: INFO,
	},
	TileSpec {
		tile: Some(Tile::Quiet),
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
	ui.scope(|ui| {
		// A solid bar takes its own lane beside the grid; the default floating bar was drawn over
		// today's column whenever the grid scrolled (captures at a3f5f71, slot EXIT #42532).
		ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
		egui::ScrollArea::vertical()
			.auto_shrink(false)
			.show(ui, |ui| {
				grid(ui, state, &drawn, today);
				if folds && quiet_count > 0 {
					quiet_row(ui, state, quiet_count);
				}
				legend(ui);
			});
	});
}

fn tiles(ui: &mut Ui, state: &mut AppState, now: i64) {
	let reads = &state.activity.reads;
	let counts: Vec<usize> = TILES
		.iter()
		.map(|spec| match spec.tile {
			None => state.repos.len(),
			Some(tile) => {
				let repos = state.repos.iter();
				repos
					.filter(|r| in_tile(tile, r, reads.get(&r.path), now))
					.count()
			}
		})
		.collect();
	// Every card takes the tallest card's content height, measured on the previous frame, so a card
	// whose label fits one line is not shorter than its wrapped neighbours (1440x900 capture at
	// 07b268e). The measure is each card's own text, so it shrinks again when the pane widens.
	let tall_id = ui.scope_id().with("activity-tile-height");
	let tallest: f32 = ui.data(|d| d.get_temp(tall_id)).unwrap_or(0.0);
	let mut measured: f32 = 0.0;
	ui.columns(TILES.len(), |cols| {
		for (i, (spec, col)) in TILES.iter().zip(cols.iter_mut()).enumerate() {
			let TileSpec {
				tile,
				label,
				note,
				colour,
			} = spec;
			let on = state.activity.filter == *tile;
			let line = if on { ACCENT_LINE.get() } else { LINE.get() };
			let frame = egui::Frame::new()
				.fill(if on { ACCENT_BG.get() } else { BG_PANEL.get() })
				.stroke(Stroke::new(1.0, line))
				.corner_radius(RADIUS)
				.inner_margin(12.0);
			let inner = frame.show(col, |ui| {
				ui.set_min_width(ui.available_width());
				ui.set_min_height(tallest);
				// Left-aligned and unjustified: `columns` hands each column a justified layout, and a
				// wrapped label there spaces its letters out to the full width (seen at 1440x900).
				let text = ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
					let number = RichText::new(counts[i].to_string());
					ui.label(number.size(28.0).color(*colour));
					ui.label(RichText::new(*label).color(TEXT_BRIGHT).strong());
					ui.label(RichText::new(*note).size(TEXT_SMALL).color(MUTED));
				});
				measured = measured.max(text.response.rect.height());
			});
			let id = col.scope_id().with(("activity-tile", i));
			let response = col.interact(inner.response.rect, id, Sense::click());
			let name = format!("{label}: {}. {note}", counts[i]);
			a11y::control(&response, Role::Button, Some(on), &name);
			// A lit filter card clears back to all; the All card always means all, so clicking it
			// while lit changes nothing.
			let next = if on { None } else { *tile };
			if response.clicked() && next != state.activity.filter {
				state.activity.filter = next;
				state.activity.cell = (0, Some(DAYS - 1));
			}
		}
	});
	if (measured - tallest).abs() > 0.5 {
		ui.data_mut(|d| d.insert_temp(tall_id, measured));
		ui.ctx().request_repaint();
	}
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
