//! The right pane: the selected repository's header (name, branch line, path, the configured
//! remotes when there are any), the Changes | History | Files | Branches switch, and the pane
//! chosen. Diffs and files arrive with their own records.

use eframe::egui::{self, FontFamily, RichText, Ui};
use repose_core::git::remote::display_url;

use super::{branches, changes, commit_box, confirm, diff, file_view, files, history};
use crate::platform::Platform;
use crate::state::{AppState, Remote, View};
use crate::theme::*;

/// One line per configured remote, `<name> · <url>`, with the push URL in parentheses when it
/// differs; nothing while the list is being read or when no remote is configured (Phillip
/// #31822: show remotes if configured), and the failure when git could not list them.
fn remotes(ui: &mut Ui, remote: Option<&Remote>) {
	match remote {
		None => {}
		Some(Ok(list)) => {
			for entry in list {
				let mut line = format!("{} · {}", entry.name, display_url(&entry.url));
				if let Some(push) = &entry.push_url {
					line.push_str(&format!(" (push {})", display_url(push)));
				}
				ui.label(
					RichText::new(line)
						.size(TEXT_SMALL)
						.color(MUTED)
						.family(FontFamily::Monospace),
				);
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
					.size(18.0)
					.color(ACCENT)
					.family(FontFamily::Monospace),
			);
			ui.label(
				RichText::new(format!("{} · Saved refs; no fetch", repo.branch_line()))
					.size(12.0)
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
			ui.add_space(8.0);
			match (&repo.status, &repo.error) {
				(_, Some(e)) => {
					ui.label(RichText::new("Status unavailable").color(DANGER));
					ui.label(RichText::new(e).size(TEXT_SMALL).color(MUTED));
					return;
				}
				(Some(s), None) => {
					let upstream = match (&s.upstream, s.ahead, s.behind) {
						(Some(u), Some(a), Some(b)) => {
							format!("↑{a} ↓{b} against {u} (saved refs)")
						}
						_ => "No upstream".to_string(),
					};
					ui.label(RichText::new(upstream).size(12.0).color(MUTED));
				}
				(None, None) => {
					ui.label(RichText::new("Reading status…").color(MUTED));
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
}
