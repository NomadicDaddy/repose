//! Right-click menus shared by the repository, commit and file rows, opened by the Menu key on
//! the selected row as well, and the external actions they offer. A failure becomes the one
//! notice line under the repository header; a success clears it.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, Popup, Response, RichText, SetOpenCommand, Ui};

use crate::platform::Platform;
use crate::state::AppState;
use crate::theme::*;

pub(super) enum Action {
	OpenFolder(PathBuf),
	Reveal(PathBuf),
	Editor {
		target: PathBuf,
		line: Option<usize>,
	},
	Terminal(PathBuf),
	Copy(String),
	OpenUrl(String),
}

/// Shows the menu on a secondary click anywhere on the row, or on the Menu key when the row is
/// the selected one; a keyboard-opened menu sits under the row rather than at the pointer, and
/// that choice is remembered for as long as it stays open.
pub(super) fn row_menu(ui: &Ui, response: &Response, selected: bool, add: impl FnOnce(&mut Ui)) {
	let ctx = ui.ctx();
	let by_key = selected && ctx.input(|i| i.key_pressed(Key::Menu));
	let opening = by_key || response.secondary_clicked();
	let flag = Popup::default_response_id(response).with("by-key");
	if opening {
		ctx.data_mut(|d| d.insert_temp(flag, by_key));
	}
	let keyed: bool = ctx.data(|d| d.get_temp(flag)).unwrap_or(false);
	let set = if opening {
		Some(SetOpenCommand::Bool(true))
	} else if response.clicked() {
		Some(SetOpenCommand::Bool(false))
	} else {
		None
	};
	let popup = Popup::menu(response).open_memory(set);
	let popup = if keyed {
		popup.at_position(response.rect.left_bottom() + egui::vec2(24.0, 0.0))
	} else {
		popup.at_pointer_fixed()
	};
	popup.show(add);
}

fn item(ui: &mut Ui, label: &str) -> bool {
	let clicked = ui.button(label).clicked();
	if clicked {
		ui.close();
	}
	clicked
}

fn disabled(ui: &mut Ui, label: &str, why: &str) {
	ui.add_enabled(false, egui::Button::new(label))
		.on_disabled_hover_text(why);
}

/// `remote` is None while the origin URL is still being read, Some(None) for no origin.
pub(super) fn repo_items(
	ui: &mut Ui,
	repo: &Path,
	remote: Option<&Option<String>>,
) -> Option<Action> {
	ui.label(
		RichText::new(repo.display().to_string())
			.size(TEXT_SMALL)
			.color(MUTED),
	);
	ui.separator();
	if item(ui, "Open folder in Explorer") {
		return Some(Action::OpenFolder(repo.to_path_buf()));
	}
	if item(ui, "Open in VS Code") {
		return Some(Action::Editor {
			target: repo.to_path_buf(),
			line: None,
		});
	}
	if item(ui, "Open a terminal here") {
		return Some(Action::Terminal(repo.to_path_buf()));
	}
	ui.separator();
	if item(ui, "Copy path") {
		return Some(Action::Copy(repo.display().to_string()));
	}
	match remote {
		None => {
			disabled(ui, "Copy remote URL", "Reading the origin remote…");
			disabled(ui, "Open remote page", "Reading the origin remote…");
		}
		Some(None) => {
			disabled(
				ui,
				"Copy remote URL",
				"This repository has no origin remote",
			);
			disabled(
				ui,
				"Open remote page",
				"This repository has no origin remote",
			);
		}
		Some(Some(url)) => {
			if item(ui, "Copy remote URL") {
				return Some(Action::Copy(url.clone()));
			}
			match repose_core::git::remote::https_page(url) {
				Some(page) => {
					if item(ui, "Open remote page") {
						return Some(Action::OpenUrl(page));
					}
				}
				None => disabled(
					ui,
					"Open remote page",
					"The origin is a local path, which has no web page",
				),
			}
		}
	}
	None
}

pub(super) fn commit_items(ui: &mut Ui, hash: &str, subject: &str) -> Option<Action> {
	if item(ui, "Copy hash") {
		return Some(Action::Copy(hash.to_string()));
	}
	if item(ui, "Copy short hash") {
		return Some(Action::Copy(hash.get(..7).unwrap_or(hash).to_string()));
	}
	if item(ui, "Copy subject") {
		return Some(Action::Copy(subject.to_string()));
	}
	None
}

/// `line` is the line the editor should open at, when the row knows one.
pub(super) fn file_items(
	ui: &mut Ui,
	repo: &Path,
	path: &str,
	line: Option<usize>,
) -> Option<Action> {
	let full = repo.join(path.replace('/', "\\"));
	if item(ui, "Open in VS Code") {
		return Some(Action::Editor {
			target: full,
			line: Some(line.unwrap_or(1)),
		});
	}
	if item(ui, "Reveal in Explorer") {
		return Some(Action::Reveal(full));
	}
	ui.separator();
	if item(ui, "Copy path") {
		return Some(Action::Copy(full.display().to_string()));
	}
	if item(ui, "Copy relative path") {
		return Some(Action::Copy(path.to_string()));
	}
	None
}

pub(super) fn perform(
	action: Action,
	state: &mut AppState,
	platform: &dyn Platform,
	ctx: &egui::Context,
) {
	let result = match action {
		Action::OpenFolder(dir) => platform.open_folder(&dir),
		Action::Reveal(file) => platform.reveal_file(&file),
		Action::Editor { target, line } => {
			platform.open_in_editor(&state.config.commands.editor, &target, line)
		}
		Action::Terminal(dir) => platform.open_terminal(&state.config.commands.terminal, &dir),
		Action::Copy(text) => {
			ctx.copy_text(text);
			Ok(())
		}
		Action::OpenUrl(url) => {
			ctx.open_url(egui::OpenUrl::new_tab(url));
			Ok(())
		}
	};
	state.notice = result.err();
}
