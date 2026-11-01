//! The diff pane: a breadcrumb, Inline or side by side, hunk separators, both line numbers, added
//! and removed backgrounds, word marks, and the binary, conflicted and too-large notices. Rows
//! are drawn only when visible, so a long diff costs what is on screen.

use eframe::egui::{self, FontFamily, Key, RichText, Sense, Ui, vec2};
use repose_core::model::{DiffSource, FileDiff};

use super::diff_draw::{ROW_H, draw_row};
use crate::diff_rows::{Row, build_rows};
use crate::platform::Platform;
use crate::state::{AppState, DiffContent, DiffState};
use crate::theme::*;

pub fn show(ui: &mut Ui, state: &mut AppState, platform: &dyn Platform) {
	if ui.input(|i| i.key_pressed(Key::Escape)) {
		state.close_diff();
		return;
	}
	let Some(d) = state.diff.as_mut() else {
		return;
	};
	let (mut go_repo, mut close) = (false, false);
	breadcrumb(ui, d, &mut go_repo, &mut close);
	if go_repo || close {
		let repo = d.open.repo.clone();
		state.close_diff();
		if go_repo {
			state.select(repo);
		}
		return;
	}
	let DiffState {
		open,
		content,
		inline,
		rows,
		rows_inline,
		..
	} = d;
	match content {
		DiffContent::Loading => {
			ui.label(RichText::new("Reading diff…").color(MUTED));
		}
		DiffContent::TooLarge(size) => {
			ui.label(
				RichText::new(format!(
					"File too large to diff ({:.1} MB)",
					*size as f64 / 1_048_576.0
				))
				.color(WARN),
			);
			if ui.button("Reveal in Explorer").clicked() {
				let _ = platform.reveal_file(&open.repo.join(open.display.replace('/', "\\")));
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
				let text = if open.untracked {
					"Untracked file: nothing to diff yet. The file view arrives with the file browser."
				} else {
					"No differences."
				};
				ui.label(RichText::new(text).color(MUTED));
				return;
			}
			if rows.len() != files.len() || *rows_inline != *inline {
				*rows = files.iter().map(|f| build_rows(f, *inline)).collect();
				*rows_inline = *inline;
			}
			for (file, file_rows) in files.iter().zip(rows.iter()) {
				file_block(ui, file, file_rows);
			}
		}
	}
}

fn breadcrumb(ui: &mut Ui, d: &mut DiffState, go_repo: &mut bool, close: &mut bool) {
	let repo_name = d
		.open
		.repo
		.file_name()
		.map(|n| n.to_string_lossy().to_string())
		.unwrap_or_default();
	let (folder, file) = match d.open.display.rsplit_once('/') {
		Some((f, n)) => (f, n),
		None => ("", d.open.display.as_str()),
	};
	let sep = |ui: &mut Ui| {
		ui.label(RichText::new("›").color(MUTED));
	};
	ui.horizontal(|ui| {
		if ui
			.link(
				RichText::new(repo_name)
					.family(FontFamily::Monospace)
					.color(ACCENT),
			)
			.clicked()
		{
			*go_repo = true;
		}
		if !folder.is_empty() {
			sep(ui);
			ui.label(RichText::new(folder).color(MUTED));
		}
		sep(ui);
		ui.label(RichText::new(file).color(TEXT_BRIGHT).strong());
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
				d.inline = !d.inline;
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

fn file_block(ui: &mut Ui, file: &FileDiff, rows: &[Row]) {
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
	egui::ScrollArea::vertical()
		.auto_shrink([false, false])
		.show_rows(ui, ROW_H, rows.len(), |ui, range| {
			let width = ui.available_width();
			for row in &rows[range] {
				let (rect, _) = ui.allocate_exact_size(vec2(width, ROW_H), Sense::hover());
				draw_row(ui, rect, file, row);
			}
		});
}
