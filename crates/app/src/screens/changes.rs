//! The Changes view: Merge Changes, Staged Changes and Changes groups with counts, letter badges,
//! and hover actions that queue writes.

use std::path::Path;

use eframe::egui::{self, Align2, FontFamily, FontId, RichText, Sense, StrokeKind, Ui, pos2, vec2};
use repose_core::model::{ChangeKind, DiffSource, DiscardPlan, FileChange, RepoSummary};

use super::menu;
use crate::platform::Platform;
use crate::state::{AppState, OpenDiff, PendingDiscard, WriteKind};
use crate::theme::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
	Conflicts,
	Staged,
	Working,
}

/// What every row in one group shares.
struct GroupCtx<'a> {
	repo: &'a RepoSummary,
	which: Group,
	/// False while a write for this repository is running or waiting.
	enabled: bool,
	platform: &'a dyn Platform,
}

pub fn show(ui: &mut Ui, state: &mut AppState, platform: &dyn Platform) {
	let Some(repo) = state.selected_repo().cloned() else {
		return;
	};
	let Some(changes) = repo.changes.clone() else {
		return;
	};
	if changes.is_empty() {
		ui.label(
			RichText::new(format!("No changes on {}", repo.branch_line()))
				.color(MUTED)
				.size(TEXT_BASE),
		);
		return;
	}
	let busy = state.writes.busy_for(&repo.path);
	let ctx = |which| GroupCtx {
		repo: &repo,
		which,
		enabled: busy.is_none(),
		platform,
	};
	if !changes.conflicts.is_empty() {
		group(
			ui,
			state,
			&ctx(Group::Conflicts),
			"Merge Changes",
			&changes.conflicts,
			busy,
		);
	}
	group(
		ui,
		state,
		&ctx(Group::Staged),
		"Staged Changes",
		&changes.staged,
		busy,
	);
	let working: Vec<FileChange> = changes.working().cloned().collect();
	group(ui, state, &ctx(Group::Working), "Changes", &working, busy);
}

fn group(
	ui: &mut Ui,
	state: &mut AppState,
	g: &GroupCtx<'_>,
	title: &str,
	files: &[FileChange],
	busy: Option<&'static str>,
) {
	ui.add_space(10.0);
	ui.horizontal(|ui| {
		ui.label(
			RichText::new(title)
				.size(TEXT_BASE)
				.color(TEXT_BRIGHT)
				.strong(),
		);
		count_pill(ui, files.len());
		if let Some(label) = busy {
			ui.label(
				RichText::new(format!("{label}…"))
					.size(TEXT_SMALL)
					.color(MUTED),
			);
		}
		ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
			let enabled = g.enabled && !files.is_empty();
			let all = match g.which {
				Group::Staged => Some(("Unstage all", WriteKind::UnstageAll)),
				Group::Working => Some(("Stage all", WriteKind::StageAll)),
				Group::Conflicts => None,
			};
			if let Some((label, kind)) = all
				&& ui.add_enabled(enabled, egui::Button::new(label)).clicked()
			{
				let ctx = ui.ctx().clone();
				state.request_write(g.repo.path.clone(), kind, &ctx);
			}
		});
	});
	if files.is_empty() {
		ui.label(RichText::new("Nothing here").size(TEXT_SMALL).color(MUTED));
		return;
	}
	for file in files {
		file_row(ui, state, g, file);
	}
}

fn count_pill(ui: &mut Ui, n: usize) {
	let font = FontId::new(TEXT_SMALL, FontFamily::Proportional);
	let galley = ui.painter().layout_no_wrap(n.to_string(), font, ACCENT);
	let size = galley.size() + vec2(12.0, 4.0);
	let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
	ui.painter().rect(
		rect,
		9.0,
		ACCENT_BG,
		egui::Stroke::new(1.0, ACCENT_LINE),
		StrokeKind::Inside,
	);
	ui.painter()
		.galley(pos2(rect.left() + 6.0, rect.top() + 2.0), galley, ACCENT);
}

fn badge_color(kind: ChangeKind) -> egui::Color32 {
	match kind {
		ChangeKind::Modified | ChangeKind::TypeChanged => WARN,
		ChangeKind::Added | ChangeKind::IntentToAdd => ACCENT,
		ChangeKind::Deleted | ChangeKind::Conflicted => DANGER,
		ChangeKind::Renamed => INFO,
		ChangeKind::Untracked => MUTED,
	}
}

fn file_row(ui: &mut Ui, state: &mut AppState, g: &GroupCtx<'_>, file: &FileChange) {
	let selected = state.selected_file.as_deref() == Some(file.path.as_str());
	let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
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
	let x = rect.left() + 8.0;
	let name_font = FontId::new(TEXT_BASE, FontFamily::Proportional);
	let name_color = if file.kind == ChangeKind::Deleted {
		MUTED
	} else {
		TEXT
	};
	let name = painter.layout_no_wrap(file.file_name().to_string(), name_font, name_color);
	let name_width = name.size().x;
	painter.galley(
		pos2(x, rect.center().y - name.size().y / 2.0),
		name,
		name_color,
	);
	if file.kind == ChangeKind::Deleted {
		let y = rect.center().y;
		painter.line_segment(
			[pos2(x, y), pos2(x + name_width, y)],
			egui::Stroke::new(1.0, MUTED),
		);
	}
	let folder = match &file.from {
		Some(from) => format!("{}  ←  {from}", file.folder()),
		None => file.folder().to_string(),
	};
	painter.text(
		pos2(x + name_width + 10.0, rect.center().y),
		Align2::LEFT_CENTER,
		folder,
		FontId::new(TEXT_SMALL, FontFamily::Proportional),
		MUTED,
	);
	painter.text(
		pos2(rect.right() - 10.0, rect.center().y),
		Align2::RIGHT_CENTER,
		file.kind.letter(),
		FontId::new(TEXT_BASE, FontFamily::Monospace),
		badge_color(file.kind),
	);
	if response.clicked() {
		state.selected_file = Some(file.path.clone());
		let source = match g.which {
			Group::Staged => DiffSource::Staged,
			Group::Working | Group::Conflicts => DiffSource::Working,
		};
		let mut paths = vec![file.path.clone()];
		if let Some(from) = &file.from {
			paths.push(from.clone());
		}
		let ctx = ui.ctx().clone();
		state.open_diff(
			OpenDiff {
				repo: g.repo.path.clone(),
				source,
				paths,
				display: file.path.clone(),
			},
			&ctx,
		);
	}
	let mut action = None;
	menu::row_menu(ui, &response, selected, |ui| {
		action = menu::file_items(ui, &g.repo.path, &file.path, None);
	});
	if let Some(a) = action {
		let ctx = ui.ctx().clone();
		menu::perform(a, state, g.platform, &ctx);
	}
	if response.hovered() || selected {
		hover_actions(ui, state, g, file, rect);
	}
}

fn hover_actions(
	ui: &mut Ui,
	state: &mut AppState,
	g: &GroupCtx<'_>,
	file: &FileChange,
	rect: egui::Rect,
) {
	let area = egui::Rect::from_min_max(
		pos2(rect.right() - 260.0, rect.top()),
		pos2(rect.right() - 28.0, rect.bottom()),
	);
	let mut child = ui.new_child(
		egui::UiBuilder::new()
			.max_rect(area)
			.layout(egui::Layout::right_to_left(egui::Align::Center)),
	);
	let ctx = child.ctx().clone();
	let (action, kind) = match g.which {
		Group::Staged => ("Unstage", WriteKind::Unstage(vec![file.path.clone()])),
		Group::Working | Group::Conflicts => ("Stage", WriteKind::Stage(vec![file.path.clone()])),
	};
	if child
		.add_enabled(g.enabled, egui::Button::new(action).small())
		.clicked()
	{
		state.request_write(g.repo.path.clone(), kind, &ctx);
	}
	if g.which == Group::Working {
		let plan = file.discard_plan();
		let allowed = !matches!(plan, DiscardPlan::Refused(_));
		let button = child.add_enabled(g.enabled && allowed, egui::Button::new("Discard").small());
		let button = match plan {
			DiscardPlan::Refused(reason) => button.on_disabled_hover_text(reason),
			_ => button,
		};
		if button.clicked() {
			let (tracked, untracked) = match plan {
				DiscardPlan::Remove => (Vec::new(), vec![file.path.clone()]),
				DiscardPlan::Restore => (vec![file.path.clone()], Vec::new()),
				DiscardPlan::Refused(_) => (Vec::new(), Vec::new()),
			};
			if !tracked.is_empty() || !untracked.is_empty() {
				state.pending_discard = Some(PendingDiscard {
					repo: g.repo.path.clone(),
					tracked,
					untracked,
				});
			}
		}
	}
	if child.add(egui::Button::new("Open").small()).clicked() {
		let full: &Path = &g.repo.path.join(file.path.replace('/', "\\"));
		let _ = g.platform.reveal_file(full);
	}
}
