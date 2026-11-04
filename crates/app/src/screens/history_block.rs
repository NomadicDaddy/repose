//! An expanded commit in the History pane: its message, its files with their change letters,
//! and the actions on them.

use eframe::egui::{self, FontFamily, RichText, Ui};
use repose_core::model::{ChangeKind, Commit, DiffSource, RepoSummary};

use crate::platform::Platform;
use crate::screens::menu;
use crate::state::{AppState, OpenDiff};
use crate::theme::*;

pub(super) fn expanded_block(
	ui: &mut Ui,
	state: &mut AppState,
	repo: &RepoSummary,
	c: &Commit,
	files: Option<Result<&[repose_core::model::CommitFile], &str>>,
	platform: &dyn Platform,
) {
	let mut action = None;
	egui::Frame::new()
		.fill(BG_PANEL)
		.inner_margin(10.0)
		.corner_radius(RADIUS)
		.show(ui, |ui| {
			if !c.body.is_empty() {
				ui.label(RichText::new(&c.body).size(TEXT_BASE).color(TEXT));
				ui.add_space(6.0);
			}
			match files {
				None => {
					ui.label(
						RichText::new("Reading files…")
							.size(TEXT_SMALL)
							.color(MUTED),
					);
				}
				Some(Err(e)) => {
					ui.label(
						RichText::new(format!("Files could not be read: {e}"))
							.size(TEXT_SMALL)
							.color(DANGER),
					);
				}
				Some(Ok([])) => {
					ui.label(
						RichText::new("No files changed")
							.size(TEXT_SMALL)
							.color(MUTED),
					);
				}
				Some(Ok(list)) => {
					// The working copy matches only this commit's files when it is HEAD; a file
					// deleted here has none (Morticia L-2 on 85ec3b0).
					let at_head = repo
						.short_hash
						.as_deref()
						.is_some_and(|s| c.hash.starts_with(s));
					for f in list {
						let absent = if f.kind == ChangeKind::Deleted {
							Some("Deleted in this commit; there is no working copy")
						} else if !at_head {
							Some("Changed at an older commit; the working copy may differ. Open it from Changes or the Files tab at HEAD.")
						} else {
							None
						};
						ui.horizontal(|ui| {
							ui.label(
								RichText::new(f.kind.letter())
									.family(FontFamily::Monospace)
									.color(letter_color(f.kind)),
							);
							let text = match &f.from {
								Some(from) => format!("{}  ←  {from}", f.path),
								None => f.path.clone(),
							};
							let row = ui.selectable_label(
								false,
								RichText::new(text).size(TEXT_BASE).color(TEXT),
							);
							menu::row_menu(ui, &row, false, |ui| {
								action = menu::file_items(ui, &repo.path, &f.path, None, absent);
							});
							if row.clicked() {
								let mut paths = vec![f.path.clone()];
								if let Some(from) = &f.from {
									paths.push(from.clone());
								}
								let ctx = ui.ctx().clone();
								state.open_diff(
									OpenDiff {
										repo: repo.path.clone(),
										source: DiffSource::Commit(c.hash.clone()),
										paths,
										display: f.path.clone(),
									},
									&ctx,
								);
							}
						});
					}
				}
			}
		});
	if let Some(a) = action {
		let ctx = ui.ctx().clone();
		menu::perform(a, state, platform, &ctx);
	}
}

fn letter_color(kind: ChangeKind) -> egui::Color32 {
	match kind {
		ChangeKind::Modified | ChangeKind::TypeChanged => WARN,
		ChangeKind::Added | ChangeKind::IntentToAdd => ACCENT,
		ChangeKind::Deleted | ChangeKind::Conflicted => DANGER,
		ChangeKind::Renamed => INFO,
		ChangeKind::Untracked => MUTED,
	}
}
