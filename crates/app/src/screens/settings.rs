//! The settings screen (settings-screen): a modal over the window, opened by Settings in the
//! repository list header, for the roots, the folder names to skip, the scan depth, the theme
//! and the editor and terminal commands, pre-filled from config.toml. A root that does not exist,
//! a folder name with a slash or a command that is not a program name or absolute path is marked
//! under its box, and Save stays disabled until each is fixed. Save writes config.toml and applies
//! the settings at once; Cancel and Escape drop the edits. A click outside does nothing, so a
//! stray click never throws away a half-made change.

use eframe::egui::{self, Key, RichText, Ui};
use repose_core::config::Theme;

use crate::state::{AppState, MAX_DEPTH, SettingsDraft};
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
	let height = (ctx.content_rect().height() - 220.0).max(160.0);
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
		check: SettingsDraft::root_problem,
		focus_new: draft.fresh,
	};
	list(ui, &roots, &mut draft.roots, &mut draft.new_root);
	heading(
		ui,
		"Skip folders named",
		"Never searched inside, at any depth.",
	);
	let excluded = Kind {
		noun: "excluded name",
		hint: "node_modules",
		check: SettingsDraft::exclude_problem,
		focus_new: false,
	};
	list(ui, &excluded, &mut draft.exclude, &mut draft.new_exclude);
	heading(
		ui,
		"Scan depth",
		"How many folders below each root to look.",
	);
	ui.horizontal(|ui| {
		let label = ui.label("Depth");
		ui.add(egui::DragValue::new(&mut draft.depth).range(1..=MAX_DEPTH))
			.labelled_by(label.id);
	});
	heading(
		ui,
		"Theme",
		"Follow Windows switches when the Windows app mode does.",
	);
	ui.horizontal(|ui| {
		ui.radio_value(&mut draft.theme, Theme::System, "Follow Windows");
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

/// What an editable list holds: its noun, the new box's hint, the check that marks an entry, and
/// whether the new box takes focus (the first list, on the frame the screen opens).
#[derive(Clone, Copy)]
struct Kind {
	noun: &'static str,
	hint: &'static str,
	check: fn(&str) -> Option<&'static str>,
	focus_new: bool,
}

/// An editable list: each entry with Remove and its problem, then a box and Add for a new one.
fn list(ui: &mut Ui, kind: &Kind, items: &mut Vec<String>, new_item: &mut String) {
	let Kind {
		noun,
		hint,
		check,
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
			problem(ui, check(item));
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
}

/// A command's box with its label and problem line.
fn command(ui: &mut Ui, label: &str, text: &mut String) {
	ui.horizontal(|ui| {
		let name = ui.add_sized([70.0, 20.0], egui::Label::new(label));
		let edit = egui::TextEdit::singleline(text)
			.desired_width(460.0)
			.font(egui::TextStyle::Monospace);
		ui.add(edit).labelled_by(name.id);
	});
	problem(ui, SettingsDraft::command_problem(text));
}
