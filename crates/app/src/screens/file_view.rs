//! The read-only file viewer: the breadcrumb with the revision, Open in VS Code for the working
//! copy when the file is shown at HEAD, line numbers and syntax colour in the diff's own row
//! painter, and the binary, too-large and empty notices. Rows are drawn only when visible.

use eframe::egui::{self, FontFamily, Key, RichText, Sense, Ui, vec2};

use super::crumb;
use super::diff_draw::{GUTTER, MONO_ADVANCE, ROW_H, draw_file_line};
use crate::platform::Platform;
use crate::state::{AppState, FileContent, FileRev, FileViewState};
use crate::theme::*;

pub fn show(ui: &mut Ui, state: &mut AppState, platform: &dyn Platform) {
	if ui.input(|i| i.key_pressed(Key::Escape)) {
		state.close_file();
		return;
	}
	let editor = state.config.commands.editor.clone();
	let Some(v) = state.file_view.as_mut() else {
		return;
	};
	let (mut go_repo, mut close, mut open_editor) = (false, false, false);
	ui.horizontal(|ui| {
		go_repo = crumb::path(ui, &v.open.repo, &v.open.path);
		ui.label(
			RichText::new(format!("@ {}", v.open.rev.label()))
				.size(TEXT_SMALL)
				.color(MUTED)
				.family(FontFamily::Monospace),
		);
		ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
			if ui.button("Close").on_hover_text("Esc").clicked() {
				close = true;
			}
			let at_head = v.open.rev == FileRev::Head;
			let button = ui.add_enabled(at_head, egui::Button::new("Open in VS Code"));
			let button = if at_head {
				button.on_hover_text(format!("{editor} --goto, at the current line"))
			} else {
				button.on_disabled_hover_text(format!(
					"The editor opens the working copy, and this is the file at {}. Back to HEAD first.",
					v.open.rev.label()
				))
			};
			if button.clicked() {
				open_editor = true;
			}
		});
	});
	if go_repo || close {
		let repo = v.open.repo.clone();
		state.close_file();
		if go_repo {
			state.select(repo);
		}
		return;
	}
	if open_editor {
		let full = v.open.repo.join(v.open.path.replace('/', "\\"));
		let line = v.current_line.unwrap_or(1);
		v.notice = platform.open_in_editor(&editor, &full, Some(line)).err();
	}
	if let Some(n) = &v.notice {
		ui.label(RichText::new(n.as_str()).size(TEXT_SMALL).color(WARN));
	}
	body(ui, v, platform);
}

fn body(ui: &mut Ui, v: &mut FileViewState, platform: &dyn Platform) {
	let FileViewState {
		open,
		content,
		current_line,
		notice,
		..
	} = v;
	match content {
		FileContent::Loading => {
			ui.label(RichText::new("Reading file…").color(MUTED));
		}
		FileContent::TooLarge(size) => {
			ui.label(
				RichText::new(format!(
					"File too large to show ({:.1} MB)",
					*size as f64 / 1_048_576.0
				))
				.color(WARN),
			);
			// Explorer can only show the working copy, which is this file at HEAD alone; a
			// failure is reported, not dropped (Morticia L-3 on e176de4).
			let at_head = open.rev == FileRev::Head;
			let button = ui.add_enabled(at_head, egui::Button::new("Reveal in Explorer"));
			let button = if at_head {
				button
			} else {
				button.on_disabled_hover_text(format!(
					"The working copy may differ from the file at {}. Back to HEAD first.",
					open.rev.label()
				))
			};
			if button.clicked() {
				*notice = platform
					.reveal_file(&open.repo.join(open.path.replace('/', "\\")))
					.err();
			}
		}
		FileContent::Binary => {
			ui.label(RichText::new("Binary file; no text view").color(MUTED));
		}
		FileContent::Error(e) => {
			ui.label(RichText::new("File unavailable").color(DANGER));
			ui.label(
				RichText::new(e.as_str())
					.size(TEXT_SMALL)
					.family(FontFamily::Monospace),
			);
		}
		FileContent::Text { lines, colours } => {
			if lines.is_empty() {
				ui.label(RichText::new("Empty file").color(MUTED));
				return;
			}
			ui.label(
				RichText::new(format!("{} lines", lines.len()))
					.size(TEXT_SMALL)
					.color(MUTED),
			);
			ui.add_space(4.0);
			// Rows are as wide as the longest line, so a line wider than the pane scrolls
			// sideways instead of being cut off (Morticia L-2 on e176de4). Width is estimated
			// from the character count at the monospace size; a line of wide glyphs may still
			// clip at its very end.
			let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
			let needed = GUTTER + 16.0 + longest as f32 * MONO_ADVANCE;
			egui::ScrollArea::both()
				.auto_shrink([false, false])
				.show_rows(ui, ROW_H, lines.len(), |ui, range| {
					let width = ui.available_width().max(needed);
					for i in range {
						let (rect, response) =
							ui.allocate_exact_size(vec2(width, ROW_H), Sense::click());
						if response.clicked() {
							*current_line = Some(i + 1);
						}
						let spans = colours.get(i).map_or(&[][..], Vec::as_slice);
						draw_file_line(
							ui,
							rect,
							(i + 1) as u32,
							&lines[i],
							spans,
							*current_line == Some(i + 1),
						);
					}
				});
		}
	}
}
