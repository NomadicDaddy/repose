//! The Branches tab: the repository's local branches, read-only, the current one first and
//! marked, with the short hash, the upstream and the saved ahead/behind pills. Nothing here
//! switches, creates, deletes or fetches (branches-list clauses 2 and 3).

use eframe::egui::{self, FontFamily, FontId, Key, RichText, Sense, Ui, pos2, vec2};
use repose_core::model::{Branch, HeadState};

use super::row::{fit, frame, pill};
use super::{a11y, fetch, menu};
use crate::state::AppState;
use crate::theme::*;

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
			a11y::loading_label(ui, "Reading branches…");
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
			"{} · {}",
			match branches.list.len() {
				1 => "1 local branch".to_string(),
				n => format!("{n} local branches"),
			},
			fetch::refs_note(state, &repo.path)
		))
		.size(TEXT_SMALL)
		.color(MUTED),
	);
	ui.add_space(4.0);
	let ids: Vec<egui::Id> = branches
		.list
		.iter()
		.map(|branch| branch_row(ui, branch))
		.collect();
	// Up and Down move focus within the list while a row holds it (clause 5); the repository
	// list reads rows_focus next frame and leaves its selection alone.
	let focused = ids.iter().position(|id| ui.memory(|m| m.has_focus(*id)));
	if let Some(index) = focused {
		state.rows_focus = true;
		let (up, down) = ui.input(|i| (i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown)));
		let next = match (up, down) {
			(true, false) => index.saturating_sub(1),
			(false, true) => (index + 1).min(ids.len() - 1),
			_ => index,
		};
		if next != index {
			ui.memory_mut(|m| m.request_focus(ids[next]));
		}
	}
}

/// Draws one row and returns the id its focus lives under.
fn branch_row(ui: &mut Ui, branch: &Branch) -> egui::Id {
	let id = ui.make_persistent_id(("branch-row", branch.name.as_str()));
	let (rect, _) = ui.allocate_exact_size(
		vec2(ui.available_width(), BRANCH_ROW_HEIGHT),
		Sense::hover(),
	);
	let response = ui.interact(rect, id, Sense::click());
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
	let line = branch.current.then_some(ACCENT_LINE);
	frame(&painter, rect, RADIUS, fill, line, response.has_focus());
	// Pills from the right, as the repository rows draw them: behind, then ahead, only when
	// non-zero; the name and the rest take what is left.
	let mut right = rect.right() - 8.0;
	if let Some(behind) = branch.behind.filter(|b| *b > 0) {
		right = pill(
			&painter,
			right,
			rect.center().y,
			&format!("↓{behind}"),
			INFO.get(),
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
	// The name keeps room for the hash and the hash stops at the pills; each is cut with an
	// ellipsis rather than running under them (audit-web-design-guidelines-1794001253).
	let hash_full = painter.layout_no_wrap(
		branch.short_hash.clone(),
		mono(TEXT_ROW_BRANCH),
		MUTED.get(),
	);
	let name_room = (right - x - hash_full.size().x - 12.0).max(0.0);
	let name = fit(
		&painter,
		&branch.name,
		mono(TEXT_ROW_NAME),
		name_color,
		name_room,
	);
	let name_w = name.size().x;
	painter.galley(
		pos2(x, rect.center().y - name.size().y / 2.0),
		name,
		name_color.get(),
	);
	x += name_w + 12.0;
	let hash = fit(
		&painter,
		&branch.short_hash,
		mono(TEXT_ROW_BRANCH),
		MUTED,
		(right - x).max(0.0),
	);
	let hash_w = hash.size().x;
	painter.galley(
		pos2(x, rect.center().y - hash.size().y / 2.0),
		hash,
		MUTED.get(),
	);
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
			MUTED.get(),
			width,
		);
		painter.galley(
			pos2(x, rect.center().y - galley.size().y / 2.0),
			galley,
			MUTED.get(),
		);
	}
	// A cut name stays readable in full on hover.
	let response = response.on_hover_text(&branch.name);
	// Read-only: the only action is copying the name (clause 3), so the row is a list item, not
	// a button.
	a11y::control(
		&response,
		egui::Role::ListItem,
		None,
		&a11y::branch_name(branch),
	);
	// Through the shared row menu, so the Menu key opens it on the focused row as it does on every
	// other row, not only a secondary click (audit-frontend-1793711157).
	let name_text = branch.name.clone();
	menu::row_menu(ui, &response, false, |ui| {
		if ui.button("Copy branch name").clicked() {
			ui.ctx().copy_text(name_text.clone());
			ui.close();
		}
	});
	id
}
