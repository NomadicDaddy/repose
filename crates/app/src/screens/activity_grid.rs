//! The Activity screen's grid (activity-screen clauses 4, 6 and 7): one row per repository, its
//! name then one cell per day in five steps from the panel colour to the accent, and a legend
//! under it. The grid is one Tab stop: arrow keys move the focus between the row names and the
//! cells (Home and End to a row's first and last day), and Enter or a click on a name or a cell
//! opens that repository's History. Every name and cell is its own node in the accessibility tree,
//! named as its tooltip reads (Roger 5Q-2 on 4051085), without adding a Tab stop.

use eframe::egui::{
	self, FontFamily, FontId, Key, Rect, RichText, Role, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use repose_core::git::activity::MAX_WALK;
use repose_core::model::RepoSummary;

use super::activity::GridRow;
use super::{a11y, count_label, row};
use crate::state::activity::{DAYS, day_label, step};
use crate::state::{AppState, View};
use crate::theme::*;

const NAME_WIDTH: f32 = 230.0;
const ROW_HEIGHT: f32 = 40.0;
const HEADER_HEIGHT: f32 = 20.0;

/// A place in the grid: the row, and the day, or None for the row's name.
type Spot = (usize, Option<usize>);

pub(super) fn grid(ui: &mut Ui, state: &mut AppState, rows: &[GridRow], today: i64) {
	let width = ui.available_width();
	let cell = ((width - NAME_WIDTH) / DAYS as f32).max(14.0);
	let height = HEADER_HEIGHT + ROW_HEIGHT * rows.len().max(1) as f32;
	let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
	// One widget, so one Tab stop; its id is kept so focus can be given to it directly.
	let id = ui.make_persistent_id("activity-grid");
	state.activity.grid_id = Some(id);
	let response = ui.interact(rect, id, Sense::click());
	let painter = ui.painter_at(rect);
	let font = FontId::new(TEXT_SMALL, FontFamily::Proportional);
	for d in 0..DAYS {
		let at = pos2(
			rect.left() + NAME_WIDTH + cell * (d as f32 + 0.5),
			rect.top() + 8.0,
		);
		let day = day_label(d, today).1.to_string();
		painter.text(
			at,
			egui::Align2::CENTER_CENTER,
			day,
			font.clone(),
			MUTED.get(),
		);
	}
	if rows.is_empty() {
		let msg = if state.repos.is_empty() {
			"No repositories listed yet."
		} else {
			"No repository matches."
		};
		let at = pos2(rect.left(), rect.top() + HEADER_HEIGHT + 12.0);
		painter.text(at, egui::Align2::LEFT_CENTER, msg, font, MUTED.get());
		return;
	}
	let focused = response.has_focus();
	if focused {
		// The list reads this next frame and leaves its selection alone, as for a Branches row;
		// otherwise an arrow here would also move the repository selection and leave Activity.
		state.rows_focus = true;
		let filter = egui::EventFilter {
			horizontal_arrows: true,
			vertical_arrows: true,
			..Default::default()
		};
		ui.memory_mut(|m| m.set_focus_lock_filter(response.id, filter));
		move_cell(ui, state, rows.len());
	}
	let (fr, fd) = state.activity.cell;
	let fr = fr.min(rows.len() - 1);
	let describe = |(i, d): Spot| match d {
		Some(d) => describe_cell(&rows[i], &day_label(d, today).0, d),
		None if rows[i].truncated => {
			let name = a11y::repo_name(&rows[i].repo);
			format!("{name}, {}, open History", truncated_note())
		}
		None => format!("{}, open History", a11y::repo_name(&rows[i].repo)),
	};
	let pointer = response.hover_pos();
	let mut hovered = None;
	let mut spot = |ui: &Ui, area: Rect, at: Spot, role: Role| {
		if focused && (fr, fd) == at {
			let ring = Stroke::new(2.0, ACCENT.get());
			painter.rect_stroke(area.expand(1.0), RADIUS, ring, StrokeKind::Outside);
		}
		if pointer.is_some_and(|p| area.contains(p)) {
			hovered = Some(at);
		}
		let node = ui.interact(area, id.with(at), Sense::hover());
		a11y::text(ui, node.id, area, role, &describe(at));
	};
	// A long name or note stops at the name column instead of running under the cells; the row's
	// tooltip and accessible name carry it whole.
	let column = Rect::from_min_max(
		rect.min,
		pos2(rect.left() + NAME_WIDTH - 4.0, rect.bottom()),
	);
	let names = painter.with_clip_rect(column);
	for (i, r) in rows.iter().enumerate() {
		let top = rect.top() + HEADER_HEIGHT + ROW_HEIGHT * i as f32;
		let mono = FontId::new(TEXT_ROW_NAME, FontFamily::Monospace);
		let name_at = pos2(rect.left(), top + 12.0);
		names.text(
			name_at,
			egui::Align2::LEFT_CENTER,
			&r.repo.name,
			mono,
			TEXT.get(),
		);
		// The branch always shows (clause 4); a failed or cut read adds why (Roger 5Q-1 on 5c2b7f0).
		let mut note = r.repo.branch_line();
		if r.failed {
			note.push_str(" · could not read");
		} else if r.truncated {
			note.push_str(&format!(" · newest {MAX_WALK} only"));
		}
		let note_at = pos2(rect.left(), top + 28.0);
		names.text(
			note_at,
			egui::Align2::LEFT_CENTER,
			note,
			font.clone(),
			MUTED.get(),
		);
		badges(
			&painter,
			&r.repo,
			rect.left() + NAME_WIDTH - 8.0,
			top + 20.0,
		);
		let name_rect = Rect::from_min_size(
			pos2(rect.left(), top + 2.0),
			vec2(NAME_WIDTH - 4.0, ROW_HEIGHT - 4.0),
		);
		spot(ui, name_rect, (i, None), Role::RowHeader);
		for d in 0..DAYS {
			let cell_rect = Rect::from_min_size(
				pos2(rect.left() + NAME_WIDTH + cell * d as f32 + 2.0, top + 6.0),
				vec2(cell - 4.0, ROW_HEIGHT - 12.0),
			);
			let fill = r.counts.map_or(BG_PANEL.get(), |c| step_colour(step(c[d])));
			let outline = Stroke::new(1.0, LINE_SOFT.get());
			painter.rect(cell_rect, RADIUS, fill, outline, StrokeKind::Inside);
			spot(ui, cell_rect, (i, Some(d)), Role::Cell);
		}
	}
	a11y::control(&response, Role::Grid, None, &describe((fr, fd)));
	if let Some(at) = hovered {
		response.clone().on_hover_text(describe(at));
		if response.clicked() {
			state.activity.cell = at;
			open_history(state, &rows[at.0].repo);
		}
	} else if focused && ui.input(|i| i.key_pressed(Key::Enter)) {
		open_history(state, &rows[fr].repo);
	}
}

/// Why a row's counts may be low: its read stopped at the walk's bound (Roger 5Q-1 on 4431bdb).
fn truncated_note() -> String {
	format!("newest {MAX_WALK} commits only")
}

/// A cell's tooltip and accessible name: "ledger-api, Tue 4 Nov, 6 commits", or "at least 6
/// commits" on a row whose read was cut short, since the true count can only be higher.
fn describe_cell(r: &GridRow, when: &str, d: usize) -> String {
	match r.counts {
		Some(c) => {
			let commits = count_label(c[d] as usize, "commit", "commits");
			let floor = if r.truncated { "at least " } else { "" };
			format!("{}, {when}, {floor}{commits}", r.repo.name)
		}
		None if r.failed => format!("{}, could not read", r.repo.name),
		None => format!("{}, reading", r.repo.name),
	}
}

/// Left from the first day reaches the row's name and Right from the name its first day; Home and
/// End go to the first and last day.
fn move_cell(ui: &Ui, state: &mut AppState, rows: usize) {
	let (mut r, mut d) = state.activity.cell;
	ui.input(|i| {
		if i.key_pressed(Key::ArrowUp) {
			r = r.saturating_sub(1);
		}
		if i.key_pressed(Key::ArrowDown) {
			r += 1;
		}
		if i.key_pressed(Key::ArrowLeft) {
			d = d.and_then(|d| d.checked_sub(1));
		}
		if i.key_pressed(Key::ArrowRight) {
			d = Some(d.map_or(0, |d| (d + 1).min(DAYS - 1)));
		}
		if i.key_pressed(Key::Home) {
			d = Some(0);
		}
		if i.key_pressed(Key::End) {
			d = Some(DAYS - 1);
		}
	});
	state.activity.cell = (r.min(rows - 1), d);
}

fn open_history(state: &mut AppState, repo: &RepoSummary) {
	state.select(repo.path.clone());
	state.view = View::History;
}

/// The change count and ahead/behind pills, right-aligned at `right`, as on the list rows.
fn badges(painter: &egui::Painter, repo: &RepoSummary, right: f32, cy: f32) {
	let Some(s) = &repo.status else { return };
	let mut right = right;
	if let Some(b) = s.behind.filter(|n| *n > 0) {
		right = row::pill(painter, right, cy, &format!("↓{b}"), INFO) - 4.0;
	}
	if let Some(a) = s.ahead.filter(|n| *n > 0) {
		right = row::pill(painter, right, cy, &format!("↑{a}"), ACCENT) - 4.0;
	}
	if s.changed() > 0 {
		row::pill(painter, right, cy, &s.changed().to_string(), WARN);
	}
}

/// Five steps from the panel colour to the accent.
fn step_colour(step: usize) -> egui::Color32 {
	let t = [0.0, 0.25, 0.5, 0.75, 1.0][step.min(4)];
	BG_PANEL.get().lerp_to_gamma(ACCENT.get(), t)
}

/// The closing row for the repositories with no commit in the grid's days: activating it opens
/// them as grid rows above it, with their cells and History action (Roger 5Q-1 on 4051085), and
/// activating it again folds them.
pub(super) fn quiet_row(ui: &mut Ui, state: &mut AppState, count: usize) {
	let open = state.activity.quiet_open;
	let arrow = if open { "▾" } else { "▸" };
	let quiet = count_label(count, "quiet repository", "quiet repositories");
	let button = egui::Button::new(RichText::new(format!("{arrow} {quiet}")).color(MUTED));
	if ui.add(button.frame(false)).clicked() {
		state.activity.quiet_open = !open;
	}
}

pub(super) fn legend(ui: &mut Ui) {
	ui.add_space(8.0);
	ui.horizontal(|ui| {
		ui.label(RichText::new("Less").size(TEXT_SMALL).color(MUTED));
		for s in 0..5 {
			let (rect, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
			let outline = Stroke::new(1.0, LINE_SOFT.get());
			ui.painter()
				.rect(rect, 2.0, step_colour(s), outline, StrokeKind::Inside);
		}
		let scale = "More · per day: 0, 1–3, 4–7, 8–15, 16+ commits";
		ui.label(RichText::new(scale).size(TEXT_SMALL).color(MUTED));
	});
}
