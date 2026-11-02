//! Right-click menus shared by the repository, commit and file rows, opened by the Menu key on
//! the selected row as well, and the external actions they offer. A failure becomes the one
//! notice line under the repository header; a success clears it.

use std::path::{Path, PathBuf};

use eframe::egui::{self, Key, Popup, Response, RichText, SetOpenCommand, Ui};
use repose_core::git::remote::{https_page, origin};

use crate::platform::Platform;
use crate::state::{AppState, Remote};
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
/// the selected one or holds keyboard focus (Tab reaches every row, so a file row with no
/// selection of its own still opens from the keyboard; Roger 5Q-2 on 85ec3b0). A
/// keyboard-opened menu sits under the row rather than at the pointer, and that choice is
/// remembered for as long as it stays open.
pub(super) fn row_menu(ui: &Ui, response: &Response, selected: bool, add: impl FnOnce(&mut Ui)) {
	let ctx = ui.ctx();
	// A modal blocks the pointer and focus but not raw key reads, so the Menu key opens nothing
	// on a row behind one; egui's own layer check, read from the previous frame (Morticia Low on
	// a2e978f).
	let live = ctx.memory(|m| m.allows_interaction(ui.layer_id()));
	let by_key =
		live && (selected || response.has_focus()) && ctx.input(|i| i.key_pressed(Key::Menu));
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

/// `remote` is None while the origin is still being read, then the URL, none, or the failure.
pub(super) fn repo_items(ui: &mut Ui, repo: &Path, remote: Option<&Remote>) -> Option<Action> {
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
	match remote.map(|r| r.as_ref().map(|list| origin(list))) {
		None => {
			disabled(ui, "Copy remote URL", "Reading the remotes…");
			disabled(ui, "Open remote page", "Reading the remotes…");
		}
		Some(Ok(None)) => {
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
		Some(Err(e)) => {
			let why = format!("The remotes could not be read: {e}");
			disabled(ui, "Copy remote URL", &why);
			disabled(ui, "Open remote page", &why);
		}
		Some(Ok(Some(entry))) => {
			let url = &entry.url;
			if item(ui, "Copy remote URL") {
				return Some(Action::Copy(url.clone()));
			}
			match https_page(url) {
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

/// `line` is the line the editor should open at, when the row knows one. `absent` is the reason
/// the working copy is not the file this row shows (a deleted file, a file at an older commit),
/// in which case the editor and Explorer items are disabled with it and only the paths can be
/// copied (Morticia L-2 on 85ec3b0, L-3 on e176de4).
pub(super) fn file_items(
	ui: &mut Ui,
	repo: &Path,
	path: &str,
	line: Option<usize>,
	absent: Option<&str>,
) -> Option<Action> {
	let full = repo.join(path.replace('/', "\\"));
	match absent {
		Some(why) => {
			disabled(ui, "Open in VS Code", why);
			disabled(ui, "Reveal in Explorer", why);
		}
		None => {
			if item(ui, "Open in VS Code") {
				return Some(Action::Editor {
					target: full,
					line: Some(line.unwrap_or(1)),
				});
			}
			if item(ui, "Reveal in Explorer") {
				return Some(Action::Reveal(full));
			}
		}
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
		Action::Terminal(dir) => {
			// The shell fallback waits for a helper process to report; that wait belongs off the
			// UI thread (Roger 5Q-1 on 17aa443). A failure arrives later as a notice.
			let command = state.config.commands.terminal.clone();
			let tx = state.sender();
			let wake = ctx.clone();
			std::thread::spawn(move || {
				let platform = crate::platform::current();
				if let Err(e) = platform.open_terminal(&command, &dir) {
					// The notice lands later, under whichever repository is selected by then, so
					// it names the full folder it was for; two roots can hold repositories of the
					// same name (Roger 5Q-3 on 740b7fd, 5Q-1 on 79ff03e).
					let _ = tx.send(crate::state::Msg::Notice(format!(
						"Terminal in {}: {e}",
						dir.display()
					)));
					wake.request_repaint();
				}
			});
			Ok(())
		}
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
