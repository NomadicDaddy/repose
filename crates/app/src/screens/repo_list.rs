//! The left pane: Repositories title, Refresh, Find a repository, the rows, and the status line.

use std::path::PathBuf;

use eframe::egui::{self, Key, RichText, Ui};
use repose_core::model::RepoSummary;

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
		.frame(egui::Frame::new().fill(BG_PANEL.get()).inner_margin(10.0))
		.show(root, |ui| {
			header(ui, state);
			ui.add_space(4.0);
			let count = state.repos.len();
			// With a filter typed, the top-level rows it leaves: a row stays when its own name or a
			// worktree's name matches, which is what visible_paths keeps.
			let matched = (!state.filter.trim().is_empty()).then(|| {
				let paths = state.visible_paths();
				state
					.repos
					.iter()
					.filter(|r| paths.contains(&r.path))
					.count()
			});
			let line = status_line(state, count, matched, true);
			let shown = ui.label(RichText::new(&line).size(TEXT_SMALL).color(MUTED));
			let quiet = &status_line(state, count, matched, false);
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
		.hint_text("name or path…")
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
	let mut ev = RowEvents::default();
	// The same eligibility as row_menu's, so a Menu press behind the discard modal starts no
	// origin read when the menu it would feed cannot open (Roger 5Q-1 on 8791260).
	let menu_key = ui.memory(|m| m.allows_interaction(ui.layer_id()))
		&& ui.input(|i| i.key_pressed(Key::Menu));
	let rows = egui::ScrollArea::vertical()
		.auto_shrink([false, false])
		.show(ui, |ui| {
			for repo in state.repos.iter().filter(|r| paths.contains(&r.path)) {
				// The chevron reads open whenever worktree rows are showing, whether the user
				// opened the row or the filter opened it to the matching ones; a click then
				// opens it fully or closes it (Phillip #31154).
				let expanded = repo.worktrees.iter().any(|w| paths.contains(&w.path));
				let parent = RowKind::Parent { expanded };
				row_events(ui, state, repo, parent, menu_key, &mut ev);
				for wt in repo.worktrees.iter().filter(|w| paths.contains(&w.path)) {
					row_events(ui, state, wt, RowKind::Worktree, menu_key, &mut ev);
				}
			}
		});
	// The list's scroll state lives in egui memory under this id; test builds publish it so the UI
	// test refresh_preserves_selection can read the offset back (live-refresh clause 5).
	#[cfg(test)]
	ui.data_mut(|d| d.insert_temp(scroll_key(), rows.id));
	#[cfg(not(test))]
	let _ = rows;
	apply_events(ui, state, platform, &paths, ev);
}

/// After the rows are drawn: the selection's reveal is spent, then the chevron, click, origin
/// read and menu action the rows recorded are applied.
fn apply_events(
	ui: &Ui,
	state: &mut AppState,
	platform: &dyn Platform,
	paths: &[PathBuf],
	ev: RowEvents,
) {
	let RowEvents {
		clicked,
		toggled,
		wants_remote,
		action,
	} = ev;
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

/// What the rows asked for this frame, applied after the list is drawn.
#[derive(Default)]
struct RowEvents {
	clicked: Option<PathBuf>,
	toggled: Option<PathBuf>,
	/// A menu opening on a row asks for that repository's origin URL; it lands a frame later.
	wants_remote: Option<PathBuf>,
	action: Option<menu::Action>,
}

/// A parent row, whose chevron shows its worktrees, or a worktree row indented under it.
#[derive(Clone, Copy)]
enum RowKind {
	Parent { expanded: bool },
	Worktree,
}

/// One row, parent or worktree: drawn, scrolled to when the selection asks, and its click,
/// chevron, menu key and menu recorded in `ev`.
fn row_events(
	ui: &mut Ui,
	state: &AppState,
	repo: &RepoSummary,
	kind: RowKind,
	menu_key: bool,
	ev: &mut RowEvents,
) {
	let is_selected = state.selected.as_deref() == Some(repo.path.as_path());
	let (indent, expanded) = match kind {
		RowKind::Parent { expanded } => (0.0, expanded),
		RowKind::Worktree => (16.0, false),
	};
	let out = row::draw(ui, state, repo, is_selected, indent, expanded);
	if is_selected && state.reveal_selected {
		out.response.scroll_to_me(Some(egui::Align::Center));
	}
	if matches!(kind, RowKind::Parent { .. }) && out.chevron_clicked {
		ev.toggled = Some(repo.path.clone());
	} else if out.response.clicked() {
		ev.clicked = Some(repo.path.clone());
	}
	if out.response.secondary_clicked() || ((is_selected || out.response.has_focus()) && menu_key) {
		ev.wants_remote = Some(repo.path.clone());
	}
	menu::row_menu(ui, &out.response, is_selected, |ui| {
		ev.action = menu::repo_items(ui, &repo.path, state);
	});
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
		&& let Some(path) = state.selected_repo().map(|repo| repo.path.clone())
	{
		// A failure is named under the repository header, as from the row menu.
		state.notice = platform.open_folder(&path).err();
	}
}
