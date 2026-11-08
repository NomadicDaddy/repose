//! The Changes view: Merge Changes, Staged Changes and Changes groups with counts, letter badges,
//! and hover actions that queue writes.

use eframe::egui::{self, Align2, FontFamily, FontId, RichText, Sense, Ui, pos2, vec2};
use repose_core::model::{ChangeKind, DiffSource, FileChange, RepoSummary};

use super::row::{fit, frame};
use super::{a11y, menu};
use crate::platform::Platform;
use crate::state::{AppState, OpenDiff, WriteKind};
use crate::theme::*;

#[path = "change_actions.rs"]
mod actions;
pub(crate) use actions::ACTIONS_LEFT;
use actions::hover_actions;

#[path = "changes_header.rs"]
mod header;
use header::{count_pill, disclosure};

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
	// Each group folds to its header, for this repository and this session; the count, the busy
	// label and the group action stay on the header (audit-frontend-1793999566).
	let fold_id = ui.make_persistent_id(("changes-group", title, g.repo.path.as_os_str()));
	let mut open = ui.data(|d| d.get_temp::<bool>(fold_id)).unwrap_or(true);
	ui.horizontal(|ui| {
		// Clickable but not a Tab stop: the header button beside it is the keyboard path, and an
		// unnamed stop before it would only confuse (Morticia Low on c505d7a).
		let (tri, tri_response) = ui.allocate_exact_size(vec2(12.0, 16.0), Sense::CLICK);
		disclosure(ui.painter(), tri.center(), open);
		let header = ui.add(
			egui::Button::new(
				RichText::new(title)
					.size(TEXT_BASE)
					.color(TEXT_BRIGHT)
					.strong(),
			)
			.frame(false),
		);
		let state_word = if open { "expanded" } else { "collapsed" };
		a11y::control(
			&header,
			egui::Role::DisclosureTriangle,
			None,
			&format!("{title}, {state_word}"),
		);
		if header.clicked() || tri_response.clicked() {
			open = !open;
			ui.data_mut(|d| d.insert_temp(fold_id, open));
		}
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
	if !open {
		return;
	}
	if files.is_empty() {
		ui.label(RichText::new("Nothing here").size(TEXT_SMALL).color(MUTED));
		return;
	}
	for file in files {
		file_row(ui, state, g, file);
	}
}

fn file_row(ui: &mut Ui, state: &mut AppState, g: &GroupCtx<'_>, file: &FileChange) {
	let selected = state.selected_file.as_deref() == Some(file.path.as_str());
	let (rect, response) = ui.allocate_exact_size(
		vec2(ui.available_width(), CHANGE_ROW_HEIGHT),
		Sense::click(),
	);
	// The row's own buttons cover it, so `hovered()` goes false under them and the actions would
	// vanish every other frame; the pointer being in the row is what keeps them drawn.
	let pointer_in = ui.rect_contains_pointer(rect);
	let painter = ui.painter_at(rect);
	let fill = if selected {
		Some(ACCENT_BG)
	} else if pointer_in {
		Some(BG_HOVER)
	} else {
		None
	};
	let line = selected.then_some(ACCENT_LINE);
	// The row's actions show while the pointer is in it or it is selected.
	let actions_shown = pointer_in || selected;
	frame(&painter, rect, RADIUS, fill, line, response.has_focus());
	let x = rect.left() + 8.0;
	let name_font = FontId::new(TEXT_BASE, FontFamily::Proportional);
	let name_color = if file.kind == ChangeKind::Deleted {
		MUTED
	} else {
		TEXT
	};
	// The name and the folder stop short of the kind letter's slot, or of the actions area while
	// the actions show (the pointer is in the row or it is selected), and are cut with an
	// ellipsis, as repository rows and commit subjects are; the tooltip carries the full path
	// (audit-web-design-guidelines-1794001253, row clipping; Roger on 78779f5).
	let text_right = if actions_shown {
		rect.right() - ACTIONS_LEFT - 6.0
	} else {
		rect.right() - 30.0
	};
	let name = fit(
		&painter,
		file.file_name(),
		name_font,
		name_color,
		(text_right - x).max(0.0),
	);
	let name_width = name.size().x;
	painter.galley(
		pos2(x, rect.center().y - name.size().y / 2.0),
		name,
		name_color.get(),
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
	let folder_x = x + name_width + 10.0;
	let folder = fit(
		&painter,
		&folder,
		FontId::new(TEXT_SMALL, FontFamily::Proportional),
		MUTED,
		(text_right - folder_x).max(0.0),
	);
	painter.galley(
		pos2(folder_x, rect.center().y - folder.size().y / 2.0),
		folder,
		MUTED.get(),
	);
	painter.text(
		pos2(rect.right() - 10.0, rect.center().y),
		Align2::RIGHT_CENTER,
		file.kind.letter(),
		FontId::new(TEXT_BASE, FontFamily::Monospace),
		super::change_kind_colour(file.kind).get(),
	);
	let tooltip = match &file.from {
		Some(from) => format!("{}\nrenamed from {from}", file.path),
		None => file.path.clone(),
	};
	let response = response.on_hover_text(tooltip);
	a11y::control(
		&response,
		egui::Role::Button,
		Some(selected),
		&a11y::change_name(file),
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
	let absent = (file.kind == ChangeKind::Deleted)
		.then_some("Deleted in the working tree; there is no file to open");
	menu::row_menu(ui, &response, selected, |ui| {
		action = menu::file_items(ui, &g.repo.path, &file.path, None, absent);
	});
	if let Some(a) = action {
		let ctx = ui.ctx().clone();
		menu::perform(a, state, g.platform, &ctx);
	}
	if actions_shown {
		hover_actions(ui, state, g, file, rect);
	}
}
