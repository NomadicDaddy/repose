//! The Branches tab: the repository's local branches, read-only, the current one first and
//! marked, with the short hash, the upstream and the saved ahead/behind pills. Nothing here
//! switches, creates, deletes or fetches (branches-list clauses 2 and 3).

use eframe::egui::{self, FontFamily, FontId, RichText, Sense, StrokeKind, Ui, pos2, vec2};
use repose_core::model::{Branch, HeadState};

use super::row::pill;
use crate::state::AppState;
use crate::theme::*;

const ROW: f32 = 32.0;

pub fn show(ui: &mut Ui, state: &mut AppState) {
	let Some(repo) = state.selected_repo().cloned() else {
		return;
	};
	let ctx = ui.ctx().clone();
	state.ensure_branches(&repo.path, &ctx);
	let Some(b) = state.branches_of(&repo.path) else {
		return;
	};
	if let Some(e) = &b.error {
		ui.label(RichText::new(e.as_str()).size(TEXT_SMALL).color(DANGER));
	}
	let Some(branches) = b.branches.clone() else {
		if b.loading {
			ui.label(RichText::new("Reading branches…").color(MUTED));
		}
		return;
	};
	match &branches.head {
		HeadState::Unborn => {
			ui.label(RichText::new("No commits yet").color(MUTED));
			return;
		}
		HeadState::Detached(hash) => {
			ui.label(
				RichText::new(format!("HEAD detached at {hash}"))
					.size(TEXT_SMALL)
					.color(WARN),
			);
		}
		HeadState::OnBranch(_) => {}
	}
	ui.label(
		RichText::new(format!(
			"{} · Saved refs; no fetch",
			match branches.list.len() {
				1 => "1 local branch".to_string(),
				n => format!("{n} local branches"),
			}
		))
		.size(TEXT_SMALL)
		.color(MUTED),
	);
	ui.add_space(4.0);
	for branch in &branches.list {
		branch_row(ui, branch);
	}
}

fn branch_row(ui: &mut Ui, branch: &Branch) {
	let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click());
	let painter = ui.painter_at(rect);
	// The same three looks as a repository row (visual-theme clause 5): the current branch has
	// the accent fill with its soft line, a hovered row the grey fill, and a row holding keyboard
	// focus (Tab reaches every row) the one-pixel bright outline over whichever fill applies.
	let fill = if branch.current {
		Some(ACCENT_BG)
	} else if response.hovered() {
		Some(BG_HOVER)
	} else {
		None
	};
	let outline = if response.has_focus() {
		Some(ACCENT)
	} else if branch.current {
		Some(ACCENT_LINE)
	} else {
		None
	};
	if fill.is_some() || outline.is_some() {
		painter.rect(
			rect,
			RADIUS,
			fill.unwrap_or(egui::Color32::TRANSPARENT),
			outline.map_or(egui::Stroke::NONE, |c| egui::Stroke::new(1.0, c)),
			StrokeKind::Inside,
		);
	}
	// Pills from the right, as the repository rows draw them: behind, then ahead, only when
	// non-zero; the name and the rest take what is left.
	let mut right = rect.right() - 8.0;
	if let Some(behind) = branch.behind.filter(|b| *b > 0) {
		right = pill(
			&painter,
			right,
			rect.center().y,
			&format!("↓{behind}"),
			INFO,
		) - 6.0;
	}
	if let Some(ahead) = branch.ahead.filter(|a| *a > 0) {
		right = pill(&painter, right, rect.center().y, &format!("↑{ahead}"), WARN) - 6.0;
	}
	if branch.current {
		right = pill(&painter, right, rect.center().y, "current", ACCENT) - 6.0;
	}
	let mono = |size: f32| FontId::new(size, FontFamily::Monospace);
	let mut x = rect.left() + 10.0;
	let name_color = if branch.current { TEXT_BRIGHT } else { TEXT };
	let name = painter.layout_no_wrap(branch.name.clone(), mono(14.0), name_color);
	let name_w = name.size().x;
	painter.galley(
		pos2(x, rect.center().y - name.size().y / 2.0),
		name,
		name_color,
	);
	x += name_w + 12.0;
	let hash = painter.layout_no_wrap(branch.short_hash.clone(), mono(11.5), MUTED);
	let hash_w = hash.size().x;
	painter.galley(pos2(x, rect.center().y - hash.size().y / 2.0), hash, MUTED);
	x += hash_w + 12.0;
	if let Some(up) = &branch.upstream {
		let text = match (branch.ahead, branch.behind) {
			(None, None) => format!("{up} · upstream gone"),
			_ => up.clone(),
		};
		let width = (right - x).max(0.0);
		let galley = painter.layout(
			text,
			FontId::new(TEXT_SMALL, FontFamily::Proportional),
			MUTED,
			width,
		);
		painter.galley(
			pos2(x, rect.center().y - galley.size().y / 2.0),
			galley,
			MUTED,
		);
	}
	// Read-only: the only action is copying the name (clause 3).
	let name_text = branch.name.clone();
	response.context_menu(|ui| {
		if ui.button("Copy branch name").clicked() {
			ui.ctx().copy_text(name_text.clone());
			ui.close();
		}
	});
}
