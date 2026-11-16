//! A change row's hover actions, drawn while the pointer is in the row: stage or unstage,
//! discard (through the confirmation) and open, each queued as a write or run on a thread.

use std::path::Path;

use eframe::egui::{self, Ui, pos2};
use repose_core::model::{DiscardPlan, FileChange, MarkerCheck};

use super::{Group, GroupCtx};
use crate::state::{AppState, PendingDiscard, WriteKind};

/// How far from the row's right edge the actions area starts; the row's text stops short of it
/// whenever the actions show.
pub(crate) const ACTIONS_LEFT: f32 = 260.0;

/// Stage's hover text while a conflicted file still holds conflict markers.
pub(crate) const MARKERS_PRESENT: &str =
	"Conflict markers remain. Use Open to resolve them in your editor first, then stage it.";
/// Stage's hover text when repose could not read the conflicted file (binary, missing, locked).
pub(crate) const MARKERS_UNCHECKED: &str =
	"repose could not check this file for conflict markers. Stage it only once it is resolved.";

pub(super) fn hover_actions(
	ui: &mut Ui,
	state: &mut AppState,
	g: &GroupCtx<'_>,
	file: &FileChange,
	rect: egui::Rect,
) {
	let area = egui::Rect::from_min_max(
		pos2(rect.right() - ACTIONS_LEFT, rect.top()),
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
	// A conflicted file stages only once its markers are gone, or when repose cannot read it, so
	// a binary conflict can still be resolved (conflict-stage-waits-for-markers, Phillip #53130).
	let markers = (g.which == Group::Conflicts).then(|| {
		g.repo
			.changes
			.as_ref()
			.map_or(MarkerCheck::Unchecked, |c| c.marker_check(&file.path))
	});
	let stage_allowed = markers != Some(MarkerCheck::Present);
	let button = child.add_enabled(
		g.enabled && stage_allowed,
		egui::Button::new(action).small(),
	);
	let button = match markers {
		Some(MarkerCheck::Present) => button.on_disabled_hover_text(MARKERS_PRESENT),
		Some(MarkerCheck::Unchecked) => button.on_hover_text(MARKERS_UNCHECKED),
		_ => button,
	};
	if button.clicked() {
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
					// The clicked Discard is the control to come back to, whatever held focus
					// (stage-and-commit; Roger 5Q-1 on bf3df80).
					return_focus: Some(button.id),
					focus_cancel: true,
				});
			}
		}
	}
	if child.add(egui::Button::new("Open").small()).clicked() {
		let full: &Path = &crate::platform::git_worktree_path(&g.repo.path, &file.path);
		// A failure is named under the repository header, as from the row menu.
		state.notice = g.platform.reveal_file(full).err();
	}
}
