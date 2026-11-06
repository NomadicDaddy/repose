//! The right pane: the selected repository's header (name, branch line, path, the configured
//! remotes when there are any), the Changes | History | Files | Branches switch, and the pane
//! chosen. Diffs and files arrive with their own records.

use eframe::egui::{self, FontFamily, RichText, Ui};
use repose_core::git::remote::{display_url, https_page};

use super::{
	a11y, branches, changes, close_guard, commit_box, confirm, diff, fetch, file_view, files,
	history,
};
use crate::platform::Platform;
use crate::state::{AppState, Remote, View};
use crate::theme::*;

/// One line per configured remote, `<name> · <url>`, with the push URL in parentheses when it
/// differs; nothing while the list is being read or when no remote is configured (Phillip
/// #31822: show remotes if configured), and the failure when git could not list them. The
/// origin line is a link to the page Open remote page opens, when https_page gives one, with
/// that page as its tooltip; every other line, and an origin with no page, is plain text
/// (header-origin-link, Phillip #33224 and #33453).
fn remotes(ui: &mut Ui, remote: Option<&Remote>) {
	match remote {
		None => {}
		Some(Ok(list)) => {
			for entry in list {
				let mut line = format!("{} · {}", entry.name, display_url(&entry.url));
				if let Some(push) = &entry.push_url {
					line.push_str(&format!(" (push {})", display_url(push)));
				}
				let text = RichText::new(line)
					.size(TEXT_SMALL)
					.family(FontFamily::Monospace);
				let page = (entry.name == "origin")
					.then(|| https_page(&entry.url))
					.flatten();
				match page {
					Some(page) => {
						let tip = format!("Open {page}");
						if ui.link(text.color(ACCENT)).on_hover_text(tip).clicked() {
							ui.ctx().open_url(egui::OpenUrl::new_tab(page));
						}
					}
					None => {
						ui.label(text.color(MUTED));
					}
				}
			}
		}
		Some(Err(e)) => {
			ui.label(
				RichText::new(format!("Remotes could not be read: {e}"))
					.size(TEXT_SMALL)
					.color(WARN),
			);
		}
	}
}

pub fn show(root: &mut Ui, state: &mut AppState, platform: &dyn Platform) {
	let ctx = root.ctx().clone();
	egui::CentralPanel::default()
		.frame(egui::Frame::new().fill(BG_APP).inner_margin(16.0))
		.show(root, |ui| {
			let Some(repo) = state.selected_repo().cloned() else {
				ui.label(RichText::new("Choose a repository to inspect it.").color(MUTED));
				return;
			};
			ui.label(
				RichText::new(&repo.name)
					.size(TEXT_DETAIL_TITLE)
					.color(ACCENT)
					.family(FontFamily::Monospace),
			);
			// The selection stays when the filter hides its row; say so, or this pane shows a
			// repository the list does not (design-polish clause 4).
			let filter = state.filter.trim();
			if !filter.is_empty() && !state.visible_paths().contains(&repo.path) {
				ui.label(
					RichText::new(format!("Hidden from the list by the filter \"{filter}\""))
						.size(TEXT_SMALL)
						.color(WARN),
				);
			}
			ui.label(
				RichText::new(format!(
					"{} · {}",
					repo.branch_line(),
					fetch::refs_note(state, &repo.path)
				))
				.size(TEXT_META)
				.color(MUTED),
			);
			ui.label(
				RichText::new(repo.path.display().to_string())
					.size(TEXT_SMALL)
					.color(MUTED)
					.family(FontFamily::Monospace),
			);
			state.ensure_remote(&repo.path, &ctx);
			remotes(ui, state.remotes.get(&repo.path));
			if let Some(n) = &state.notice {
				ui.label(RichText::new(n.as_str()).size(TEXT_SMALL).color(WARN));
			}
			fetch::outcome(ui, state, &repo.path);
			ui.add_space(8.0);
			match (&repo.status, &repo.error) {
				(_, Some(e)) => {
					let shown = ui.label(RichText::new("Status unavailable").color(DANGER));
					a11y::live(&shown, "Status unavailable");
					ui.label(RichText::new(e).size(TEXT_SMALL).color(MUTED));
					return;
				}
				(Some(s), None) => {
					let upstream = match (&s.upstream, s.ahead, s.behind) {
						(Some(u), Some(a), Some(b)) => {
							format!("↑{a} ↓{b} against {u} (saved refs)")
						}
						// A gone upstream keeps its name and has no counts: porcelain v2 prints
						// branch.upstream without branch.ab (core git/parse/porcelain.rs).
						(Some(u), None, None) => format!("{u} · upstream gone"),
						_ => "No upstream".to_string(),
					};
					ui.label(RichText::new(upstream).size(TEXT_META).color(MUTED));
				}
				(None, None) => {
					let shown = ui.label(RichText::new("Reading status…").color(MUTED));
					a11y::live(&shown, "Reading status…");
					return;
				}
			}
			ui.add_space(10.0);
			if state.diff.is_some() {
				diff::show(ui, state, platform);
				return;
			}
			if state.file_view.is_some() {
				file_view::show(ui, state, platform);
				return;
			}
			ui.horizontal(|ui| {
				for (view, label) in [
					(View::Changes, "Changes"),
					(View::History, "History"),
					(View::Files, "Files"),
					(View::Branches, "Branches"),
				] {
					let selected = state.view == view;
					if ui.selectable_label(selected, label).clicked() {
						state.view = view;
					}
				}
			});
			egui::ScrollArea::vertical()
				.auto_shrink([false, false])
				.show(ui, |ui| match state.view {
					View::Changes => {
						commit_box::show(ui, state);
						changes::show(ui, state, platform);
					}
					View::History => history::show(ui, state, platform),
					View::Files => files::show(ui, state, platform),
					View::Branches => branches::show(ui, state),
				});
		});
	confirm::show(&ctx, state);
	close_guard::show(&ctx, state);
}
