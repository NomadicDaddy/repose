//! An expanded commit in the History pane: its message, its files with their change letters,
//! and the actions on them.

use eframe::egui::{self, FontFamily, RichText, Ui};
use repose_core::model::{ChangeKind, Commit, CommitFile, DiffSource, RepoSummary};

use crate::platform::Platform;
use crate::screens::menu;
use crate::state::{AppState, OpenDiff};
use crate::theme::*;

pub(super) fn expanded_block(
	ui: &mut Ui,
	state: &mut AppState,
	repo: &RepoSummary,
	c: &Commit,
	files: Option<Result<&[CommitFile], &str>>,
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
			let Some(list) = files_status(ui, files) else {
				return;
			};
			// The working copy matches only this commit's files when it is HEAD; a file deleted
			// here has none (Morticia L-2 on 85ec3b0).
			let at_head = repo
				.short_hash
				.as_deref()
				.is_some_and(|s| c.hash.starts_with(s));
			for f in list {
				if let Some(a) = commit_file_row(ui, state, repo, c, f, at_head) {
					action = Some(a);
				}
			}
		});
	if let Some(a) = action {
		let ctx = ui.ctx().clone();
		menu::perform(a, state, platform, &ctx);
	}
}

/// The status line for files still loading, unreadable or empty, or the list to draw.
fn files_status<'a>(
	ui: &mut Ui,
	files: Option<Result<&'a [CommitFile], &str>>,
) -> Option<&'a [CommitFile]> {
	let (text, colour) = match files {
		None => ("Reading files…".to_string(), MUTED),
		Some(Err(e)) => (format!("Files could not be read: {e}"), DANGER),
		Some(Ok([])) => ("No files changed".to_string(), MUTED),
		Some(Ok(list)) => return Some(list),
	};
	ui.label(RichText::new(text).size(TEXT_SMALL).color(colour));
	None
}

/// One changed file of an expanded commit: its letter, its path (with the old name when
/// renamed), the file menu, and a click that opens this commit's diff of the file.
fn commit_file_row(
	ui: &mut Ui,
	state: &mut AppState,
	repo: &RepoSummary,
	c: &Commit,
	f: &CommitFile,
	at_head: bool,
) -> Option<menu::Action> {
	let absent = if f.kind == ChangeKind::Deleted {
		Some("Deleted in this commit; there is no working copy")
	} else if !at_head {
		Some(
			"Changed at an older commit; the working copy may differ. Open it from Changes or the Files tab at HEAD.",
		)
	} else {
		None
	};
	let mut action = None;
	ui.horizontal(|ui| {
		// egui centres each widget on the row height known when it is placed, and the letter comes
		// before the taller, padded path label, so it sat 4.5 px high. Give the row the path
		// label's height first, and the letter the path's size (design-polish clause 6).
		let path_font = egui::FontId::proportional(TEXT_BASE);
		let row_h = ui.fonts_mut(|fonts| fonts.row_height(&path_font))
			+ 2.0 * ui.spacing().button_padding.y;
		ui.set_min_height(row_h);
		ui.label(
			RichText::new(f.kind.letter())
				.family(FontFamily::Monospace)
				.size(TEXT_BASE)
				.color(crate::screens::change_kind_colour(f.kind)),
		);
		let text = match &f.from {
			Some(from) => format!("{}  ←  {from}", f.path),
			None => f.path.clone(),
		};
		let row = ui.selectable_label(false, RichText::new(text).size(TEXT_BASE).color(TEXT));
		menu::row_menu(ui, &row, false, |ui| {
			action = menu::file_items(ui, &repo.path, &f.path, None, absent);
		});
		if row.clicked() {
			let paths = std::iter::once(f.path.clone())
				.chain(f.from.clone())
				.collect();
			let open = OpenDiff {
				repo: repo.path.clone(),
				source: DiffSource::Commit(c.hash.clone()),
				paths,
				display: f.path.clone(),
			};
			let ctx = ui.ctx().clone();
			state.open_diff(open, &ctx);
		}
	});
	action
}
