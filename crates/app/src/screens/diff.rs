//! The diff pane: a breadcrumb, Inline or side by side, hunk separators, both line numbers, added
//! and removed backgrounds, word marks, and the binary, conflicted and too-large notices. Rows
//! are drawn only when visible, so a long diff costs what is on screen.

use eframe::egui::{self, FontFamily, Key, RichText, Sense, Ui, vec2};
use repose_core::model::DiffSource;

use super::diff_draw::{Source, describe_row, draw_row};
use super::{a11y, crumb};
use crate::diff_rows::Row;
use crate::platform::Platform;
use crate::state::{AppState, DiffContent, DiffState};
use crate::theme::*;

pub fn show(ui: &mut Ui, state: &mut AppState, platform: &dyn Platform) {
	// Escape closes the diff only when no dialog or menu above it is the one answering it.
	let escape_is_ours = !state.modal_open() && !egui::Popup::is_any_open(ui.ctx());
	if escape_is_ours && ui.input(|i| i.key_pressed(Key::Escape)) {
		state.close_diff();
		return;
	}
	let Some(d) = state.diff.as_mut() else {
		return;
	};
	let (mut go_repo, mut close, mut toggle) = (false, false, false);
	breadcrumb(ui, d, &mut go_repo, &mut close, &mut toggle);
	if go_repo || close {
		let repo = d.open.repo.clone();
		state.close_diff();
		if go_repo {
			state.select(repo);
		}
		return;
	}
	if toggle {
		let ctx = ui.ctx().clone();
		state.toggle_inline(&ctx);
	}
	let Some(d) = state.diff.as_mut() else {
		return;
	};
	let DiffState {
		open,
		content,
		inline,
		rows,
		rows_inline,
		colours,
		..
	} = d;
	match content {
		DiffContent::Loading => {
			a11y::loading_label(ui, "Reading diff…");
		}
		DiffContent::TooLarge(size) => {
			ui.label(
				RichText::new(format!(
					"File too large to diff ({})",
					crate::file_tree::size_label(*size)
				))
				.color(WARN),
			);
			// A failed Explorer spawn is named under the repository header, as from the row menu.
			if ui.button("Reveal in Explorer").clicked() {
				state.notice = platform
					.reveal_file(&crate::platform::git_worktree_path(
						&open.repo,
						&open.display,
					))
					.err();
			}
		}
		DiffContent::Error(e) => {
			ui.label(RichText::new("Diff unavailable").color(DANGER));
			ui.label(
				RichText::new(e.as_str())
					.size(TEXT_SMALL)
					.family(FontFamily::Monospace),
			);
		}
		DiffContent::Files(files) => {
			if files.is_empty() {
				let text = match open.source {
					DiffSource::Working => {
						"No differences against the index. An untracked file has no diff until it is staged."
					}
					_ => "No differences.",
				};
				ui.label(RichText::new(text).color(MUTED));
				return;
			}
			// Rows are built on the reader thread (open, reload and the Inline toggle alike), so
			// the UI thread never runs the word diff; until they arrive the old layout stands.
			if rows.len() != files.len() || *rows_inline != *inline {
				let shown = ui.label(RichText::new("Laying out…").size(TEXT_SMALL).color(MUTED));
				a11y::live(&shown, "Laying out…");
			}
			for (i, (file, file_rows)) in files.iter().zip(rows.iter()).enumerate() {
				let src = Source {
					file,
					colours: colours.get(i),
				};
				file_block(ui, &src, file_rows);
			}
		}
	}
}

fn breadcrumb(
	ui: &mut Ui,
	d: &mut DiffState,
	go_repo: &mut bool,
	close: &mut bool,
	toggle: &mut bool,
) {
	ui.horizontal(|ui| {
		*go_repo = crumb::path(ui, &d.open.repo, &d.open.display);
		ui.label(
			RichText::new(source_label(&d.open.source))
				.size(TEXT_SMALL)
				.color(MUTED),
		);
		ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
			if ui.button("Close").on_hover_text("Esc").clicked() {
				*close = true;
			}
			if ui.selectable_label(d.inline, "Inline").clicked() {
				*toggle = true;
			}
		});
	});
}

fn source_label(source: &DiffSource) -> String {
	match source {
		DiffSource::Working => "working tree against index".to_string(),
		DiffSource::Staged => "index against HEAD".to_string(),
		DiffSource::Commit(h) => format!("{} against its first parent", h.get(..7).unwrap_or(h)),
	}
}

fn file_block(ui: &mut Ui, src: &Source<'_>, rows: &[Row]) {
	let file = src.file;
	ui.horizontal(|ui| {
		if file.is_rename() {
			ui.label(
				RichText::new(format!(
					"renamed from {}",
					file.old_path.as_deref().unwrap_or("")
				))
				.size(TEXT_SMALL)
				.color(INFO),
			);
		}
		ui.label(
			RichText::new(format!("+{}", file.added()))
				.size(TEXT_SMALL)
				.color(ACCENT),
		);
		ui.label(
			RichText::new(format!("−{}", file.removed()))
				.size(TEXT_SMALL)
				.color(DANGER),
		);
	});
	if file.binary {
		ui.label(RichText::new("Binary file; no text diff").color(MUTED));
		return;
	}
	if file.conflicted {
		ui.label(
			RichText::new("Conflicted: resolve it in your editor. The combined diff is not shown.")
				.color(WARN),
		);
		return;
	}
	if rows.is_empty() {
		ui.label(RichText::new("No text changes (mode or similarity only)").color(MUTED));
		return;
	}
	ui.add_space(4.0);
	let out = egui::ScrollArea::vertical()
		.auto_shrink([false, false])
		.show_rows(ui, DIFF_ROW_HEIGHT, rows.len(), |ui, range| {
			let width = ui.available_width();
			for row in &rows[range] {
				let (rect, slot) =
					ui.allocate_exact_size(vec2(width, DIFF_ROW_HEIGHT), Sense::hover());
				draw_row(ui, rect, src, row);
				describe_row(ui, slot.id, rect, src, row);
			}
		});
	// Test builds publish the id for the UI test refresh_preserves_selection (live-refresh
	// clause 5); with several files in one diff it names the last block's.
	#[cfg(test)]
	ui.data_mut(|d| d.insert_temp(scroll_key(), out.id));
	#[cfg(not(test))]
	let _ = out;
}

/// Where the diff pane publishes its rows' scroll area id each frame, in test builds.
#[cfg(test)]
pub(crate) fn scroll_key() -> egui::Id {
	egui::Id::unique("diff-rows-scroll")
}
