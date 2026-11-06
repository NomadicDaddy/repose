//! The Files pane: the repository's tree at HEAD, or at the commit opened in History, as
//! collapsible folders with file sizes; a click opens the file read-only in the viewer.

use eframe::egui::{self, FontFamily, RichText, Ui};
use repose_core::model::{BranchState, TreeEntry};

use super::menu::{self, Action};
use crate::file_tree::{Folder, files_label, size_label};
use crate::platform::Platform;
use crate::state::{AppState, FileRev, FilesState, OpenFile};
use crate::theme::*;

/// What a walk of the tree collected: the file to open and the menu action to perform.
#[derive(Default)]
struct Picked {
	open: Option<TreeEntry>,
	action: Option<Action>,
}

/// What every row of one walk shares.
struct Walk<'a> {
	salt: &'a (std::path::PathBuf, String),
	repo: &'a std::path::Path,
	/// Why the working copy is not this file, at a revision other than HEAD.
	absent: Option<&'a str>,
}

pub fn show(ui: &mut Ui, state: &mut AppState, platform: &dyn Platform) {
	let Some(repo) = state.selected_repo().cloned() else {
		return;
	};
	if repo.branch == BranchState::Unborn {
		ui.label(RichText::new("No files at HEAD").color(MUTED));
		return;
	}
	let ctx = ui.ctx().clone();
	state.ensure_files(&repo.path, &ctx);
	let Some(f) = state.files_of(&repo.path) else {
		return;
	};
	let mut back_to_head = false;
	header(ui, f, &mut back_to_head);
	if back_to_head {
		state.set_files_rev(&repo.path, FileRev::Head);
		return;
	}
	if let Some(e) = &f.error {
		ui.label(RichText::new("Files unavailable").color(DANGER));
		ui.label(
			RichText::new(e.as_str())
				.size(TEXT_SMALL)
				.family(FontFamily::Monospace),
		);
		return;
	}
	if f.loaded.as_ref() != Some(&f.rev) {
		ui.label(RichText::new("Reading tree…").color(MUTED));
		return;
	}
	if f.tree.file_count() == 0 {
		ui.label(RichText::new(format!("No files at {}", f.rev.label())).color(MUTED));
		return;
	}
	let rev = f.rev.clone();
	let salt = (repo.path.clone(), rev.label().to_string());
	let mut picked = Picked::default();
	// At an older commit the working copy is not this file, so the editor and Explorer items
	// stay off and only the paths can be copied.
	let absent = (rev != FileRev::Head).then(|| {
		format!(
			"Shown at {}; the working copy may differ or be gone. Back to HEAD to open it.",
			rev.label()
		)
	});
	let walk = Walk {
		salt: &salt,
		repo: &repo.path,
		absent: absent.as_deref(),
	};
	folder_body(ui, &f.tree, &walk, &mut picked);
	if let Some(entry) = picked.open {
		state.open_file(
			OpenFile {
				repo: repo.path.clone(),
				rev,
				path: entry.path,
				size: Some(entry.size),
			},
			&ctx,
		);
	}
	if let Some(a) = picked.action {
		menu::perform(a, state, platform, &ctx);
	}
}

fn header(ui: &mut Ui, f: &FilesState, back_to_head: &mut bool) {
	ui.add_space(6.0);
	ui.horizontal(|ui| {
		ui.label(
			RichText::new("Files")
				.size(TEXT_BASE)
				.color(TEXT_BRIGHT)
				.strong(),
		);
		ui.label(RichText::new("@").size(TEXT_SMALL).color(MUTED));
		let colour = match f.rev {
			FileRev::Head => ACCENT,
			FileRev::Commit(_) => INFO,
		};
		ui.label(
			RichText::new(f.rev.label())
				.family(FontFamily::Monospace)
				.color(colour),
		);
		if f.loaded.as_ref() == Some(&f.rev) && f.error.is_none() {
			ui.label(
				RichText::new(files_label(f.tree.file_count()))
					.size(TEXT_SMALL)
					.color(MUTED),
			);
		}
		if f.rev != FileRev::Head {
			ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
				if ui
					.button("Back to HEAD")
					.on_hover_text("Show the tree as the current commit holds it")
					.clicked()
				{
					*back_to_head = true;
				}
			});
		}
	});
	ui.add_space(4.0);
}

/// Subfolders first, then files, each in git's order. A click or a menu choice lands in `picked`.
fn folder_body(ui: &mut Ui, folder: &Folder, walk: &Walk<'_>, picked: &mut Picked) {
	for child in &folder.folders {
		let header =
			egui::CollapsingHeader::new(RichText::new(&child.name).size(TEXT_BASE).color(TEXT))
				.id_salt((walk.salt, &child.path))
				.default_open(false);
		header.show(ui, |ui| folder_body(ui, child, walk, picked));
	}
	for file in &folder.files {
		file_row(ui, file, walk, picked);
	}
}

fn file_row(ui: &mut Ui, file: &TreeEntry, walk: &Walk<'_>, picked: &mut Picked) {
	ui.horizontal(|ui| {
		ui.add_space(18.0);
		if file.submodule {
			ui.add_enabled(
				false,
				egui::Label::new(RichText::new(file.file_name()).size(TEXT_BASE).color(MUTED)),
			)
			.on_disabled_hover_text("A submodule: open it as its own repository");
			ui.label(RichText::new("submodule").size(TEXT_SMALL).color(MUTED));
			return;
		}
		let label = ui.selectable_label(false, RichText::new(file.file_name()).size(TEXT_BASE));
		if label.clicked() {
			picked.open = Some(file.clone());
		}
		menu::row_menu(ui, &label, false, |ui| {
			picked.action = menu::file_items(ui, walk.repo, &file.path, None, walk.absent);
		});
		ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
			ui.add_space(8.0);
			ui.label(
				RichText::new(size_label(file.size))
					.size(TEXT_SMALL)
					.color(MUTED)
					.family(FontFamily::Monospace),
			);
		});
	});
}
