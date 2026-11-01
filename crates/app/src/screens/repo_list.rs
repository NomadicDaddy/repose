//! The left pane: Repositories title, Refresh, Find a repository, the rows, and the status line.

use std::path::PathBuf;

use eframe::egui::{self, Key, RichText, Ui};

use super::{menu, row};
use crate::platform::Platform;
use crate::state::AppState;
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
			ui.label(
				RichText::new(status_line(state, count))
					.size(TEXT_SMALL)
					.color(MUTED),
			);
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
	let edit = egui::TextEdit::singleline(&mut state.filter)
		.hint_text("Find a repository")
		.desired_width(f32::INFINITY);
	let response = ui.add(edit);
	if state.focus_filter {
		response.request_focus();
		state.focus_filter = false;
	}
	if response.has_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
		state.filter.clear();
	}
}

fn status_line(state: &AppState, count: usize) -> String {
	if state.scanning && state.checked_at.is_none() {
		return "Scanning…".to_string();
	}
	let time = state
		.checked_at
		.map(|t| clock(t, state.utc_offset))
		.unwrap_or_default();
	let noun = if count == 1 {
		"repository"
	} else {
		"repositories"
	};
	let missing = match state.missing_roots.len() {
		0 => String::new(),
		1 => format!(" · 1 root not found: {}", state.missing_roots[0].display()),
		n => format!(" · {n} roots not found"),
	};
	// The watcher's notes: a root it could not watch or no watcher at all (lasting), and Watcher
	// caught up after an overflow (until the next full scan completes).
	let mut note = state
		.watching
		.note
		.as_deref()
		.map(|n| format!(" · {n}"))
		.unwrap_or_default();
	if state.watching.caught_up && !state.watching.full_pending {
		note.push_str(" · Watcher caught up");
	}
	if state.scanning {
		format!("{count} {noun} · Checking… · Saved refs; no fetch{missing}{note}")
	} else {
		format!("{count} {noun} · Checked {time} · Saved refs; no fetch{missing}{note}")
	}
}

/// Local wall-clock HH:MM: UTC seconds plus the offset the last scan read, or UTC marked as such
/// when the offset is unknown. Nothing here touches the OS, so the UI thread stays free.
fn clock(t: std::time::SystemTime, offset: Option<i32>) -> String {
	let secs = t
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs())
		.unwrap_or(0);
	let Some(offset) = offset else {
		let minutes = (secs as i64).rem_euclid(86_400) / 60;
		return format!("{:02}:{:02} UTC", minutes / 60, minutes % 60);
	};
	let local = secs as i64 + i64::from(offset) * 60;
	let minutes = local.rem_euclid(86_400) / 60;
	format!("{:02}:{:02}", minutes / 60, minutes % 60)
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
				let out = row::draw(ui, repo, is_selected, 0.0, expanded);
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
					action = menu::repo_items(ui, &repo.path, state.remotes.get(&repo.path));
				});
				for wt in &repo.worktrees {
					if !paths.contains(&wt.path) {
						continue;
					}
					let is_selected = selected.as_deref() == Some(wt.path.as_path());
					let out = row::draw(ui, wt, is_selected, 16.0, false);
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
						action = menu::repo_items(ui, &wt.path, state.remotes.get(&wt.path));
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
	if let Some(p) = toggled
		&& !state.expanded.remove(&p)
	{
		state.expanded.insert(p);
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
	let (down, up, enter, find) = ctx.input(|i| {
		(
			i.key_pressed(Key::ArrowDown),
			i.key_pressed(Key::ArrowUp),
			i.key_pressed(Key::Enter),
			i.modifiers.command && i.key_pressed(Key::F),
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
	if down {
		state.move_selection(1);
	}
	if up {
		state.move_selection(-1);
	}
	if enter
		&& !typing
		&& let Some(repo) = state.selected_repo()
	{
		let _ = platform.open_folder(&repo.path);
	}
}
