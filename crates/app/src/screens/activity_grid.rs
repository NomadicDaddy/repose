//! The Activity screen's grid (activity-screen clauses 4, 6 and 7): one row per repository, one
//! cell per day in five steps from the panel colour to the accent, with the quiet repositories
//! folded and a legend under it. The grid is one Tab stop; arrow keys, Home and End move the
//! focused cell, Enter or a click opens that repository's History, and the focused cell's text is
//! its accessible name.

use eframe::egui::{
	self, FontFamily, FontId, Key, Rect, RichText, Role, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use repose_core::model::RepoSummary;

use super::activity::GridRow;
use super::{a11y, row};
use crate::state::activity::{DAYS, day_label, step};
use crate::state::{AppState, View};
use crate::theme::*;

const NAME_WIDTH: f32 = 230.0;
const ROW_HEIGHT: f32 = 40.0;
const HEADER_HEIGHT: f32 = 20.0;

pub(super) fn grid(ui: &mut Ui, state: &mut AppState, rows: &[GridRow], now: i64, offset: i32) {
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
		let x = rect.left() + NAME_WIDTH + cell * (d as f32 + 0.5);
		let day = day_label(d, now, offset).1.to_string();
		let at = pos2(x, rect.top() + 8.0);
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
	let mut hovered = None;
	for (i, r) in rows.iter().enumerate() {
		let top = rect.top() + HEADER_HEIGHT + ROW_HEIGHT * i as f32;
		let mono = FontId::new(TEXT_ROW_NAME, FontFamily::Monospace);
		let name_at = pos2(rect.left(), top + 12.0);
		painter.text(
			name_at,
			egui::Align2::LEFT_CENTER,
			&r.repo.name,
			mono,
			TEXT.get(),
		);
		let note = if r.failed {
			"could not read".to_string()
		} else {
			r.repo.branch_line()
		};
		let note_at = pos2(rect.left(), top + 28.0);
		painter.text(
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
		for d in 0..DAYS {
			let cell_rect = Rect::from_min_size(
				pos2(rect.left() + NAME_WIDTH + cell * d as f32 + 2.0, top + 6.0),
				vec2(cell - 4.0, ROW_HEIGHT - 12.0),
			);
			let fill = r.counts.map_or(BG_PANEL.get(), |c| step_colour(step(c[d])));
			let outline = Stroke::new(1.0, LINE_SOFT.get());
			painter.rect(cell_rect, RADIUS, fill, outline, StrokeKind::Inside);
			if focused && i == fr && d == fd {
				let ring = Stroke::new(2.0, ACCENT.get());
				painter.rect_stroke(cell_rect.expand(1.0), RADIUS, ring, StrokeKind::Outside);
			}
			if response.hover_pos().is_some_and(|p| cell_rect.contains(p)) {
				hovered = Some((i, d));
			}
		}
	}
	let describe = |i: usize, d: usize| describe(&rows[i], &day_label(d, now, offset).0, d);
	let fr = fr.min(rows.len() - 1);
	a11y::control(&response, Role::Grid, None, &describe(fr, fd));
	if let Some((i, d)) = hovered {
		response.clone().on_hover_text(describe(i, d));
		if response.clicked() {
			state.activity.cell = (i, d);
			open_history(state, &rows[i].repo);
		}
	} else if focused && ui.input(|i| i.key_pressed(Key::Enter)) {
		open_history(state, &rows[fr].repo);
	}
}

/// A cell's tooltip and accessible name: "ledger-api, Tue 4 Nov, 6 commits".
fn describe(r: &GridRow, when: &str, d: usize) -> String {
	match r.counts {
		Some(c) => {
			let s = if c[d] == 1 { "" } else { "s" };
			format!("{}, {when}, {} commit{s}", r.repo.name, c[d])
		}
		None if r.failed => format!("{}, could not read", r.repo.name),
		None => format!("{}, reading", r.repo.name),
	}
}

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
			d = d.saturating_sub(1);
		}
		if i.key_pressed(Key::ArrowRight) {
			d += 1;
		}
		if i.key_pressed(Key::Home) {
			d = 0;
		}
		if i.key_pressed(Key::End) {
			d = DAYS - 1;
		}
	});
	state.activity.cell = (r.min(rows - 1), d.min(DAYS - 1));
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

pub(super) fn quiet_row(ui: &mut Ui, state: &mut AppState, quiet: &[GridRow]) {
	let open = state.activity.quiet_open;
	let arrow = if open { "▾" } else { "▸" };
	let noun = if quiet.len() == 1 {
		"repository"
	} else {
		"repositories"
	};
	let label = format!("{arrow} {} quiet {noun}", quiet.len());
	let button = egui::Button::new(RichText::new(label).color(MUTED)).frame(false);
	if ui.add(button).clicked() {
		state.activity.quiet_open = !open;
	}
	if open {
		for r in quiet {
			let line = format!("{} · {}", r.repo.name, r.repo.branch_line());
			let text = RichText::new(line).family(FontFamily::Monospace);
			ui.label(text.size(TEXT_MONO).color(MUTED));
		}
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
