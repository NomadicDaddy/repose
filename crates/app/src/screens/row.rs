//! One repository row: name, branch line, badges, and the worktree chevron.

use eframe::egui::{self, FontFamily, FontId, Rect, Sense, StrokeKind, Ui, pos2, vec2};
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
	// Three states a reader must tell apart (visual-theme clause 5): the selected row has the
	// accent fill with its soft line, a hovered row the grey fill alone, and the row holding
	// keyboard focus (Tab reaches every row) a one-pixel bright accent outline, drawn over
	// whichever fill applies so a selected row with focus still reads as both.
	let fill = if selected {
		Some(ACCENT_BG)
	} else if response.hovered() {
		Some(BG_HOVER)
	} else {
		None
	};
	let outline = if response.has_focus() {
		Some(ACCENT)
	} else if selected {
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
	let x = rect.left() + 10.0 + indent;
	let name_color = if repo.error.is_some() {
		MUTED
	} else {
		TEXT_BRIGHT
	};
	// Pills and the chevron first, from the right; the name and branch line then take what is
	// left and are cut with an ellipsis rather than run under the pills or off the panel (seen
	// on worktree rows with long branch names, 2026-11-01).
	let mut right = rect.right() - 8.0;
	for (text, color) in badges(repo) {
		right = pill(&painter, right, rect.center().y, &text, color) - 6.0;
	}
	let mut text_right = right;
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
		text_right = pos.x;
		let hit = Rect::from_min_size(pos, galley.size()).expand(4.0);
		painter.galley(pos, galley, INFO);
		let r = ui.interact(
			hit,
			ui.scope_id().with((repo.path.as_os_str(), "chevron")),
			Sense::click(),
		);
		// The chevron is in the Tab order too, so it shows the same focus outline as a row.
		if r.has_focus() {
			painter.rect_stroke(hit, 3.0, egui::Stroke::new(1.0, ACCENT), StrokeKind::Inside);
		}
		chevron_clicked = r.clicked();
	}
	let width = (text_right - 8.0 - x).max(0.0);
	let name = fit(
		&painter,
		&repo.name,
		FontId::new(14.0, FontFamily::Monospace),
		name_color,
		width,
	);
	painter.galley(pos2(x, rect.top() + 7.0), name, name_color);
	let branch = fit(
		&painter,
		&repo.branch_line(),
		FontId::new(11.5, FontFamily::Monospace),
		MUTED,
		width,
	);
	painter.galley(pos2(x, rect.top() + 26.0), branch, MUTED);
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

/// One line of text laid out to fit `width`: whole when it fits, otherwise cut at a character
/// boundary with an ellipsis so it never runs under the pills or off the panel.
fn fit(
	painter: &egui::Painter,
	text: &str,
	font: FontId,
	color: egui::Color32,
	width: f32,
) -> std::sync::Arc<egui::Galley> {
	let whole = painter.layout_no_wrap(text.to_string(), font.clone(), color);
	if whole.size().x <= width {
		return whole;
	}
	// Both row fonts are monospace, so the mean advance says about how many characters fit
	// beside the ellipsis; the loop steps down from that guess, a few layouts for a uniform face
	// and never one per character of the whole text (Roger Low on cd6c406). When even the
	// ellipsis does not fit, nothing is drawn rather than something that overlaps the pills
	// (Roger on 881f590).
	let ellipsis = painter.layout_no_wrap("…".to_string(), font.clone(), color);
	if ellipsis.size().x > width {
		return painter.layout_no_wrap(String::new(), font, color);
	}
	let advance = (whole.size().x / text.chars().count().max(1) as f32).max(1.0);
	let mut keep = ((width - ellipsis.size().x) / advance).floor().max(0.0) as usize;
	loop {
		let cut: String = text.chars().take(keep).collect::<String>() + "…";
		let galley = painter.layout_no_wrap(cut, font.clone(), color);
		if galley.size().x <= width {
			return galley;
		}
		if keep == 0 {
			return ellipsis;
		}
		keep -= 1;
	}
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
