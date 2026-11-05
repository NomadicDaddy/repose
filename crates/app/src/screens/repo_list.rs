//! The left pane: Repositories title, Refresh, Find a repository, the rows, and the status line.

use std::path::PathBuf;

use eframe::egui::{self, Key, RichText, Ui};

use super::status_line::status_line;
use super::{a11y, menu, row};
use crate::platform::Platform;
use crate::state::{AppState, find};
use crate::theme::*;

pub fn show(root: &mut Ui, state: &mut AppState, platform: &dyn Platform) {
	let ctx = root.ctx().clone();
	keyboard(&ctx, state, platform);
	egui::Panel::left("repositories")
		.exact_size(SIDEBAR_WIDTH)
		.resizable(false)
		.frame(egui::Frame::new().fill(BG_PANEL).inner_margin(10.0))
		.show(root, |ui| {
			header(ui, state);
			ui.add_space(4.0);
			let count = state.repos.len();
			let line = status_line(state, count, true);
			let shown = ui.label(RichText::new(&line).size(TEXT_SMALL).color(MUTED));
			let quiet = &status_line(state, count, false);
			a11y::live_beside(ui, shown.id.with("live"), shown.rect, quiet);
			ui.add_space(6.0);
			list(ui, state, platform);
		});
}

fn header(ui: &mut Ui, state: &mut AppState) {
	ui.horizontal(|ui| {
		ui.label(
			RichText::new("Repositories")
				.size(TEXT_TITLE)
				.color(TEXT_BRIGHT)
				.strong(),
		);
		ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
			let refresh = ui.add_enabled(!state.scanning, egui::Button::new("Refresh"));
			if refresh.clicked() {
				let ctx = ui.ctx().clone();
				state.start_scan(&ctx);
			}
		});
	});
	let label = ui.label("Find a repository");
	let edit = egui::TextEdit::singleline(&mut state.filter)
		.hint_text("name or path...")
		.desired_width(f32::INFINITY);
	let response = ui
		.add(edit)
		.labelled_by(label.id)
		.accessible_name("Find a repository");
	if state.focus_filter {
		response.request_focus();
		state.focus_filter = false;
	}
	if response.has_focus() {
		state.text_focus = true;
	}
	// egui's text box answers Escape by giving focus up before this line runs, so on that frame
	// the box has lost focus rather than holding it; seen on screen 2026-11-02 (Escape left
	// the filter standing), covered by ui_filter_tests.rs.
	let cleared =
		(response.has_focus() || response.lost_focus()) && ui.input(|i| i.key_pressed(Key::Escape));
	if cleared {
		state.filter.clear();
	}
	if response.changed() || cleared {
		state.filter_changed();
	}
}

/// Where the list publishes its scroll area's id each frame, in test builds.
#[cfg(test)]
pub(crate) fn scroll_key() -> egui::Id {
	egui::Id::unique("repo-list-scroll")
}

fn list(ui: &mut Ui, state: &mut AppState, platform: &dyn Platform) {
	let paths = state.visible_paths();
	if paths.is_empty() && !state.filter.trim().is_empty() {
		ui.label(
			RichText::new(format!("No repositories match {}", state.filter.trim()))
				.color(MUTED)
				.size(TEXT_BASE),
		);
		return;
	}
	let selected = state.selected.clone();
	let mut clicked: Option<PathBuf> = None;
	let mut toggled: Option<PathBuf> = None;
	let mut action: Option<menu::Action> = None;
	// A menu opening on a row asks for that repository's origin URL; it lands a frame later.
	let mut wants_remote: Option<PathBuf> = None;
	// The same eligibility as row_menu's, so a Menu press behind the discard modal starts no
	// origin read when the menu it would feed cannot open (Roger 5Q-1 on 8791260).
	let menu_key = ui.memory(|m| m.allows_interaction(ui.layer_id()))
		&& ui.input(|i| i.key_pressed(Key::Menu));
	let rows = egui::ScrollArea::vertical()
		.auto_shrink([false, false])
		.show(ui, |ui| {
			for repo in &state.repos {
				if !paths.contains(&repo.path) {
					continue;
				}
				// The chevron reads open whenever worktree rows are showing, whether the user
				// opened the row or the filter opened it to the matching ones; a click then
				// opens it fully or closes it (Phillip #31154).
				let expanded = repo.worktrees.iter().any(|w| paths.contains(&w.path));
				let is_selected = selected.as_deref() == Some(repo.path.as_path());
				let out = row::draw(ui, state, repo, is_selected, 0.0, expanded);
				if is_selected && state.reveal_selected {
					out.response.scroll_to_me(Some(egui::Align::Center));
				}
				if out.chevron_clicked {
					toggled = Some(repo.path.clone());
				} else if out.response.clicked() {
					clicked = Some(repo.path.clone());
				}
				if out.response.secondary_clicked()
					|| ((is_selected || out.response.has_focus()) && menu_key)
				{
					wants_remote = Some(repo.path.clone());
				}
				menu::row_menu(ui, &out.response, is_selected, |ui| {
					action = menu::repo_items(ui, &repo.path, state);
				});
				for wt in &repo.worktrees {
					if !paths.contains(&wt.path) {
						continue;
					}
					let is_selected = selected.as_deref() == Some(wt.path.as_path());
					let out = row::draw(ui, state, wt, is_selected, 16.0, false);
					if is_selected && state.reveal_selected {
						out.response.scroll_to_me(Some(egui::Align::Center));
					}
					if out.response.clicked() {
						clicked = Some(wt.path.clone());
					}
					if out.response.secondary_clicked()
						|| ((is_selected || out.response.has_focus()) && menu_key)
					{
						wants_remote = Some(wt.path.clone());
					}
					menu::row_menu(ui, &out.response, is_selected, |ui| {
						action = menu::repo_items(ui, &wt.path, state);
					});
				}
			}
		});
	// The list's scroll state lives in egui memory under this id; test builds publish it so the UI
	// test refresh_preserves_selection can read the offset back (live-refresh clause 5).
	#[cfg(test)]
	ui.data_mut(|d| d.insert_temp(scroll_key(), rows.id));
	#[cfg(not(test))]
	let _ = rows;
	// The selected row was scrolled to this frame if it asked; the next move asks again.
	state.reveal_selected = false;
	// A click on the chevron closes a row whose worktrees are showing, whether the user or the
	// filter opened it, and opens a row whose are not; the entry overrides the filter from then
	// on, so the rows always change (Morticia L-6 on 4dc97d1).
	if let Some(p) = toggled {
		let showing = find(&state.repos, &p)
			.is_some_and(|r| r.worktrees.iter().any(|w| paths.contains(&w.path)));
		state.expanded.insert(p, !showing);
	}
	let ctx = ui.ctx().clone();
	if let Some(p) = clicked {
		state.ensure_remote(&p, &ctx);
		state.select(p);
	}
	if let Some(p) = wants_remote {
		state.ensure_remote(&p, &ctx);
	}
	if let Some(a) = action {
		menu::perform(a, state, platform, &ctx);
	}
}

fn keyboard(ctx: &egui::Context, state: &mut AppState, platform: &dyn Platform) {
	// The discard modal blocks pointer input and focus, not raw key reads, so while it stands no
	// key may move the selection, open Explorer or a row menu behind it (Morticia Low on a2e978f).
	if state.pending_discard.is_some() {
		return;
	}
	let typing = ctx.memory(|m| m.focused().is_some());
	// A text box that had focus on the last frame owns this frame's letters; the boxes set the
	// flag again as they draw (Phillip #31623).
	let text_focus = std::mem::replace(&mut state.text_focus, false);
	// A Branches row that had focus on the last frame owns the arrows this frame; the tab sets
	// the flag again as it draws (branches-list clause 5).
	let rows_focus = std::mem::replace(&mut state.rows_focus, false);
	let (down, up, enter, escape, find, typed) = ctx.input(|i| {
		let typed: String = i
			.events
			.iter()
			.filter_map(|e| match e {
				egui::Event::Text(t) => Some(t.as_str()),
				_ => None,
			})
			.collect();
		(
			i.key_pressed(Key::ArrowDown),
			i.key_pressed(Key::ArrowUp),
			i.key_pressed(Key::Enter),
			i.key_pressed(Key::Escape),
			i.modifiers.command && i.key_pressed(Key::F),
			typed,
		)
	});
	if find {
		state.focus_filter = true;
	}
	// While a row's menu is open the arrows belong to it; moving the selection underneath would
	// leave the menu on the old row (Morticia L-4 on 85ec3b0).
	if egui::Popup::is_any_open(ctx) {
		return;
	}
	// Letters belong to the list only when neither a text box nor a Branches row holds focus;
	// with a branch row focused they would jump the repository selection and swap the tab's
	// branches out from under the reader (Morticia Low on 9d07b17).
	if !text_focus && !rows_focus {
		if escape {
			state.type_ahead_clear();
		}
		if !typed.is_empty() {
			state.type_ahead(&typed, std::time::Instant::now());
		}
	}
	if down && !rows_focus {
		state.move_selection(1);
	}
	if up && !rows_focus {
		state.move_selection(-1);
	}
	if enter
		&& !typing
		&& let Some(repo) = state.selected_repo()
	{
		let _ = platform.open_folder(&repo.path);
	}
}
