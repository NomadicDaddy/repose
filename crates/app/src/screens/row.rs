//! One repository row: name, branch line, badges, and the worktree chevron.

use eframe::egui::{self, Align2, FontFamily, FontId, Rect, Sense, StrokeKind, Ui, pos2, vec2};
use repose_core::model::RepoSummary;

use crate::theme::*;

pub struct RowOut {
	pub response: egui::Response,
	pub chevron_clicked: bool,
}

pub fn draw(
	ui: &mut Ui,
	repo: &RepoSummary,
	selected: bool,
	indent: f32,
	expanded: bool,
) -> RowOut {
	let (rect, response) =
		ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
	let painter = ui.painter_at(rect);
	if selected {
		painter.rect(
			rect,
			RADIUS,
			ACCENT_BG,
			egui::Stroke::new(1.0, ACCENT_LINE),
			StrokeKind::Inside,
		);
	} else if response.hovered() {
		painter.rect_filled(rect, RADIUS, BG_HOVER);
	}
	let x = rect.left() + 10.0 + indent;
	let name_color = if repo.error.is_some() {
		MUTED
	} else {
		TEXT_BRIGHT
	};
	painter.text(
		pos2(x, rect.top() + 7.0),
		Align2::LEFT_TOP,
		&repo.name,
		FontId::new(14.0, FontFamily::Monospace),
		name_color,
	);
	painter.text(
		pos2(x, rect.top() + 26.0),
		Align2::LEFT_TOP,
		repo.branch_line(),
		FontId::new(11.5, FontFamily::Monospace),
		MUTED,
	);
	let mut right = rect.right() - 8.0;
	for (text, color) in badges(repo) {
		right = pill(&painter, right, rect.center().y, &text, color) - 6.0;
	}
	let mut chevron_clicked = false;
	if !repo.worktrees.is_empty() {
		let label = format!(
			"{} {}",
			repo.worktrees.len(),
			if repo.worktrees.len() == 1 {
				"worktree"
			} else {
				"worktrees"
			}
		);
		let glyph = if expanded { "▾" } else { "▸" };
		let text = format!("{glyph} {label}");
		let size = FontId::new(TEXT_SMALL, FontFamily::Proportional);
		let galley = painter.layout_no_wrap(text, size, INFO);
		let w = galley.size().x;
		let pos = pos2(right - w, rect.center().y - galley.size().y / 2.0);
		let hit = Rect::from_min_size(pos, galley.size()).expand(4.0);
		painter.galley(pos, galley, INFO);
		let r = ui.interact(
			hit,
			ui.scope_id().with((repo.path.as_os_str(), "chevron")),
			Sense::click(),
		);
		chevron_clicked = r.clicked();
	}
	let tooltip = tooltip_text(repo);
	let response = response.on_hover_text(tooltip);
	RowOut {
		response,
		chevron_clicked,
	}
}

fn badges(repo: &RepoSummary) -> Vec<(String, egui::Color32)> {
	let mut out = Vec::new();
	if repo.error.is_some() {
		out.push(("error".to_string(), DANGER));
		return out;
	}
	let Some(s) = &repo.status else {
		return out;
	};
	if let Some(b) = s.behind.filter(|b| *b > 0) {
		out.push((format!("↓{b}"), INFO));
	}
	if let Some(a) = s.ahead.filter(|a| *a > 0) {
		out.push((format!("↑{a}"), WARN));
	}
	if s.conflicts > 0 {
		out.push((format!("{} conflicts", s.conflicts), DANGER));
	}
	let changed = s.changed() - s.conflicts;
	if changed > 0 {
		out.push((changed.to_string(), WARN));
	}
	out
}

/// Draws a small pill ending at `right`; returns its left edge.
fn pill(painter: &egui::Painter, right: f32, cy: f32, text: &str, color: egui::Color32) -> f32 {
	let font = FontId::new(TEXT_SMALL, FontFamily::Proportional);
	let galley = painter.layout_no_wrap(text.to_string(), font, color);
	let size = galley.size() + vec2(12.0, 4.0);
	let rect = Rect::from_min_size(pos2(right - size.x, cy - size.y / 2.0), size);
	painter.rect(
		rect,
		9.0,
		BG_APP,
		egui::Stroke::new(1.0, color.gamma_multiply(0.6)),
		StrokeKind::Inside,
	);
	painter.galley(pos2(rect.left() + 6.0, rect.top() + 2.0), galley, color);
	rect.left()
}

fn tooltip_text(repo: &RepoSummary) -> String {
	let mut lines = vec![repo.path.display().to_string()];
	match &repo.status {
		Some(s) => {
			lines.push(format!(
				"{} staged · {} changed · {} untracked · {} conflicts",
				s.staged, s.unstaged, s.untracked, s.conflicts
			));
			match (&s.upstream, s.ahead, s.behind) {
				(Some(u), Some(a), Some(b)) => {
					lines.push(format!("↑{a} ↓{b} from saved refs of {u}"))
				}
				_ => lines.push("No upstream".to_string()),
			}
		}
		None => lines.push("Status not read yet".to_string()),
	}
	if let Some(e) = &repo.error {
		lines.push(e.clone());
	}
	if repo.submodules > 0 {
		lines.push(format!("{} submodules", repo.submodules));
	}
	lines.join("\n")
}
