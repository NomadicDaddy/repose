//! The settings screen (settings-screen): a modal over the window, opened by Settings in the
//! repository list header, for the roots, the folder names to skip, the scan depth, the theme
//! and the editor and terminal commands, pre-filled from the config in use. A new root that does
//! not exist, a folder name with a slash or a command that is not a program name or absolute path
//! is marked under its box and keeps Save disabled until fixed; a saved root that is missing now
//! is only noted, since the load tolerates an unplugged drive. Text typed in a New box but never
//! added also holds Save. Save writes config.toml and applies the settings at once; Cancel and
//! Escape drop the edits. A click outside does nothing, so a stray click never throws away a
//! half-made change.

use eframe::egui::{self, Key, RichText, Ui};
use repose_core::config::Theme;

use crate::state::{AppState, MAX_DEPTH, Mark, SettingsDraft};
use crate::theme::*;

/// What the buttons asked for this frame.
enum Outcome {
	Save,
	Cancel,
}

pub fn show(ctx: &egui::Context, state: &mut AppState) {
	let Some(draft) = state.settings.as_mut() else {
		return;
	};
	let mut outcome = None;
	let frame = egui::Frame::popup(&ctx.global_style()).inner_margin(16.0);
	let height = (ctx.content_rect().height() - 180.0).max(160.0);
	egui::Modal::new(egui::Id::unique("settings-screen"))
		.frame(frame)
		.show(ctx, |ui| {
			ui.set_width(600.0);
			ui.label(
				RichText::new("Settings")
					.size(TEXT_TITLE)
					.color(TEXT_BRIGHT)
					.strong(),
			);
			ui.add_space(8.0);
			egui::ScrollArea::vertical()
				.max_height(height)
				.auto_shrink([false, true])
				.show(ui, |ui| fields(ui, draft));
			draft.fresh = false;
			if let Some(error) = &draft.error {
				ui.add_space(6.0);
				ui.label(RichText::new(error).color(DANGER));
			}
			ui.add_space(10.0);
			ui.horizontal(|ui| {
				let save = egui::Button::new(RichText::new("Save").color(BG_APP)).fill(ACCENT);
				if ui.add_enabled(draft.ready(), save).clicked() {
					outcome = Some(Outcome::Save);
				}
				if ui.button("Cancel").clicked() {
					outcome = Some(Outcome::Cancel);
				}
			});
		});
	if outcome.is_none() && ctx.input(|i| i.key_pressed(Key::Escape)) {
		outcome = Some(Outcome::Cancel);
	}
	match outcome {
		Some(Outcome::Save) => state.save_settings(ctx),
		Some(Outcome::Cancel) => state.close_settings(),
		None => {}
	}
}

fn fields(ui: &mut Ui, draft: &mut SettingsDraft) {
	heading(ui, "Roots", "Folders searched for repositories.");
	let roots = Kind {
		noun: "root",
		hint: "D:\\path\\to\\folder",
		focus_new: draft.fresh,
	};
	let marks: Vec<_> = draft.roots.iter().map(|r| draft.root_mark(r)).collect();
	list(ui, &roots, &marks, &mut draft.roots, &mut draft.new_root);
	heading(
		ui,
		"Skip folders named",
		"Never searched inside, at any depth.",
	);
	let excluded = Kind {
		noun: "excluded name",
		hint: "node_modules",
		focus_new: false,
	};
	let marks: Vec<_> = draft
		.exclude
		.iter()
		.map(|e| SettingsDraft::exclude_mark(e))
		.collect();
	list(
		ui,
		&excluded,
		&marks,
		&mut draft.exclude,
		&mut draft.new_exclude,
	);
	// Depth and theme take one row each, with their help beside or under the pointer, so that the
	// commands show without scrolling at 1920x1080 (Morticia on the clause-4 frames, #41069).
	ui.add_space(10.0);
	ui.horizontal(|ui| {
		let label = ui.label(RichText::new("Scan depth").color(TEXT_BRIGHT).strong());
		// A depth the file already held outside the range is shown and kept as it is; only a drag
		// or a typed value is bounded (Morticia 5Q-1 on 25e8094).
		let depth = egui::DragValue::new(&mut draft.depth)
			.range(1..=MAX_DEPTH)
			.clamp_existing_to_range(false);
		ui.add(depth).labelled_by(label.id);
		ui.label(
			RichText::new("folders below each root to look in")
				.size(TEXT_SMALL)
				.color(MUTED),
		);
	});
	ui.add_space(10.0);
	ui.horizontal(|ui| {
		ui.label(RichText::new("Theme").color(TEXT_BRIGHT).strong());
		ui.radio_value(&mut draft.theme, Theme::System, "Follow Windows")
			.on_hover_text("Switches between dark and light when Windows' app mode changes.");
		ui.radio_value(&mut draft.theme, Theme::Dark, "Dark");
		ui.radio_value(&mut draft.theme, Theme::Light, "Light");
	});
	heading(
		ui,
		"Commands",
		"A program name on the PATH, or an absolute path.",
	);
	command(ui, "Editor", &mut draft.editor);
	command(ui, "Terminal", &mut draft.terminal);
}

/// A section title and the one line that says what it is for.
fn heading(ui: &mut Ui, title: &str, help: &str) {
	ui.add_space(10.0);
	ui.label(RichText::new(title).color(TEXT_BRIGHT).strong());
	ui.label(RichText::new(help).size(TEXT_SMALL).color(MUTED));
	ui.add_space(4.0);
}

/// One problem line under a box, in the danger colour.
fn problem(ui: &mut Ui, text: Option<&str>) {
	if let Some(text) = text {
		ui.label(RichText::new(text).size(TEXT_SMALL).color(DANGER));
	}
}

/// A list entry's mark: danger when it keeps Save disabled, warning when it is only a note.
fn mark(ui: &mut Ui, mark: Option<Mark>) {
	if let Some(Mark { text, blocks }) = mark {
		let colour = if blocks { DANGER } else { WARN };
		ui.label(RichText::new(text).size(TEXT_SMALL).color(colour));
	}
}

/// What an editable list holds: its noun, the new box's hint, and whether the new box takes
/// focus (the first list, on the frame the screen opens).
#[derive(Clone, Copy)]
struct Kind {
	noun: &'static str,
	hint: &'static str,
	focus_new: bool,
}

/// An editable list: each entry with Remove and its mark (`marks`, one per entry), then a box
/// and Add for a new one.
fn list(
	ui: &mut Ui,
	kind: &Kind,
	marks: &[Option<Mark>],
	items: &mut Vec<String>,
	new_item: &mut String,
) {
	let Kind {
		noun,
		hint,
		focus_new,
	} = *kind;
	let mut remove = None;
	for (i, item) in items.iter().enumerate() {
		ui.push_id((noun, i), |ui| {
			ui.horizontal(|ui| {
				ui.label(
					RichText::new(item.as_str())
						.family(egui::FontFamily::Monospace)
						.size(TEXT_MONO)
						.color(TEXT),
				);
				ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
					let button = ui.button("Remove").on_hover_text(format!("Remove {item}"));
					if button.clicked() {
						remove = Some(i);
					}
				});
			});
			mark(ui, marks.get(i).copied().flatten());
		});
	}
	if let Some(i) = remove {
		items.remove(i);
	}
	if items.is_empty() {
		ui.label(
			RichText::new(format!("No {noun}s."))
				.size(TEXT_SMALL)
				.color(MUTED),
		);
	}
	ui.horizontal(|ui| {
		let name = format!("New {noun}");
		let edit = egui::TextEdit::singleline(new_item)
			.hint_text(hint)
			.desired_width(460.0)
			.font(egui::TextStyle::Monospace);
		let response = ui.add(edit).accessible_name(name.as_str());
		if focus_new {
			response.request_focus();
		}
		let typed = !new_item.trim().is_empty();
		let enter = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
		let add = ui.add_enabled(typed, egui::Button::new("Add"));
		if typed && (add.clicked() || enter) {
			items.push(new_item.trim().to_string());
			new_item.clear();
		}
	});
	// Save waits while text sits here unadded, so it is never dropped (Morticia 5Q-2).
	if !new_item.trim().is_empty() {
		let note = format!("Press Add to keep this {noun}, or clear the box, before saving.");
		ui.label(RichText::new(note).size(TEXT_SMALL).color(MUTED));
	}
}

/// A command's box with its label and problem line.
fn command(ui: &mut Ui, label: &str, text: &mut String) {
	ui.horizontal(|ui| {
		// A fixed-width, left-aligned label column, so Editor and Terminal line up (add_sized
		// centres a shorter label; Morticia on the 0798387 frames, #41100).
		let column = egui::Layout::left_to_right(egui::Align::Center);
		let name = ui
			.allocate_ui_with_layout(egui::vec2(70.0, 20.0), column, |ui| {
				ui.set_min_width(70.0);
				ui.label(label)
			})
			.inner;
		let edit = egui::TextEdit::singleline(text)
			.desired_width(460.0)
			.font(egui::TextStyle::Monospace);
		ui.add(edit).labelled_by(name.id);
	});
	problem(ui, SettingsDraft::command_problem(text));
}
