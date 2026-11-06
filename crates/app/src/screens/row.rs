//! One repository row: name, branch line, badges, a running fetch, and the worktree chevron.

use eframe::egui::{self, FontFamily, FontId, Rect, Sense, StrokeKind, Ui, pos2, vec2};
use repose_core::model::RepoSummary;

use super::a11y;
use crate::state::AppState;
use crate::theme::*;

pub struct RowOut {
	pub response: egui::Response,
	pub chevron_clicked: bool,
}

pub fn draw(
	ui: &mut Ui,
	state: &AppState,
	repo: &RepoSummary,
	selected: bool,
	indent: f32,
	expanded: bool,
) -> RowOut {
	let (rect, response) =
		ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
	let painter = ui.painter_at(rect);
	let fill = if selected {
		Some(ACCENT_BG)
	} else if response.hovered() {
		Some(BG_HOVER)
	} else {
		None
	};
	let line = selected.then_some(ACCENT_LINE);
	frame(&painter, rect, RADIUS, fill, line, response.has_focus());
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
	// A fetch shows on its row from the click until git exits (remote-sync clause 2).
	let fetching = state.writes.fetching(&repo.path);
	if fetching {
		right = pill(&painter, right, rect.center().y, "fetching…", INFO) - 6.0;
	}
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
		// The chevron is painted, not a font glyph: the ▸ glyph came out small and above the text's
		// baseline (design-polish clause 5). A triangle about half the text size, centred on the
		// lowercase body of "<n> worktrees", pointing right when closed and down when open.
		let size = FontId::new(TEXT_SMALL, FontFamily::Proportional);
		let galley = painter.layout_no_wrap(label, size, INFO);
		let tri = TEXT_SMALL * 0.55;
		let gap = 4.0;
		let w = tri + gap + galley.size().x;
		let pos = pos2(
			right - galley.size().x,
			rect.center().y - galley.size().y / 2.0,
		);
		let baseline = galley.rows.first().and_then(|r| {
			r.row
				.glyphs
				.first()
				.map(|g| (r.pos.y + g.pos.y, g.font_ascent))
		});
		let centre_y = match baseline {
			Some((base, ascent)) => pos.y + base - ascent / 3.0,
			None => pos.y + galley.size().y / 2.0,
		};
		let cx = pos.x - gap - tri / 2.0;
		let half = tri / 2.0;
		let points = if expanded {
			vec![
				pos2(cx - half, centre_y - half * 0.6),
				pos2(cx + half, centre_y - half * 0.6),
				pos2(cx, centre_y + half * 0.6),
			]
		} else {
			vec![
				pos2(cx - half * 0.6, centre_y - half),
				pos2(cx - half * 0.6, centre_y + half),
				pos2(cx + half * 0.6, centre_y),
			]
		};
		let start = pos2(right - w, pos.y);
		text_right = start.x;
		let hit = Rect::from_min_max(start, pos + galley.size()).expand(4.0);
		painter.add(egui::Shape::convex_polygon(
			points,
			INFO,
			egui::Stroke::NONE,
		));
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
		let name = a11y::chevron_name(expanded, repo.worktrees.len());
		a11y::control(&r, egui::Role::DisclosureTriangle, None, &name);
		chevron_clicked = r.clicked();
	}
	let width = (text_right - 8.0 - x).max(0.0);
	let name = fit(
		&painter,
		&repo.name,
		FontId::new(TEXT_ROW_NAME, FontFamily::Monospace),
		name_color,
		width,
	);
	painter.galley(pos2(x, rect.top() + 7.0), name, name_color);
	let branch = fit(
		&painter,
		&repo.branch_line(),
		FontId::new(TEXT_ROW_BRANCH, FontFamily::Monospace),
		MUTED,
		width,
	);
	painter.galley(pos2(x, rect.top() + 26.0), branch, MUTED);
	let tooltip = tooltip_text(repo);
	let response = response.on_hover_text(tooltip);
	let mut spoken = a11y::repo_name(repo);
	if fetching {
		spoken.push_str(", fetching");
	}
	a11y::control(&response, egui::Role::Button, Some(selected), &spoken);
	RowOut {
		response,
		chevron_clicked,
	}
}

/// The looks every list row shares, so a reader can tell them apart (visual-theme clause 5,
/// keyboard-row-focus): `fill` is the row's selected, open, current or hovered fill, `line` its
/// own soft line when it has one, and a row holding keyboard focus (Tab reaches it) gets a
/// one-pixel bright accent outline in place of that line, drawn over whichever fill applies so a
/// selected row with focus still reads as both. Repository, branch, change and commit rows and
/// file-view lines all paint through here.
pub(super) fn frame(
	painter: &egui::Painter,
	rect: Rect,
	radius: f32,
	fill: Option<egui::Color32>,
	line: Option<egui::Color32>,
	focused: bool,
) {
	let outline = if focused { Some(ACCENT) } else { line };
	if fill.is_some() || outline.is_some() {
		painter.rect(
			rect,
			radius,
			fill.unwrap_or(egui::Color32::TRANSPARENT),
			outline.map_or(egui::Stroke::NONE, |c| egui::Stroke::new(1.0, c)),
			StrokeKind::Inside,
		);
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
/// boundary with an ellipsis so it never runs under the pills or off the panel. History's commit
/// subjects use it too, in a proportional face, where the first guess may keep a little less
/// than fits but never more.
pub(super) fn fit(
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
	// The mean advance says about how many characters fit beside the ellipsis (closely for the
	// repository rows' monospace face, roughly for History's proportional subjects); the loop
	// steps down from that guess, a few layouts and never one per character of the whole text
	// (Roger Low on cd6c406). When even the ellipsis does not fit, nothing is drawn rather than
	// something that overlaps the pills (Roger on 881f590).
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

/// Draws a small pill ending at `right`; returns its left edge. The Branches tab draws the same
/// pills for its rows.
pub(super) fn pill(
	painter: &egui::Painter,
	right: f32,
	cy: f32,
	text: &str,
	color: egui::Color32,
) -> f32 {
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
				(Some(u), None, None) => lines.push(format!("{u} · upstream gone")),
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
