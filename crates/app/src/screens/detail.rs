//! The right pane: the selected repository's header, a Changes | History switch, and the pane
//! chosen. Diffs and files arrive with their own records.

use eframe::egui::{self, FontFamily, RichText, Ui};

use super::{changes, commit_box, confirm, history};
use crate::platform::Platform;
use crate::state::{AppState, View};
use crate::theme::*;

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
			ui.horizontal(|ui| {
				for (view, label) in [(View::Changes, "Changes"), (View::History, "History")] {
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
					View::History => history::show(ui, state),
				});
		});
	confirm::show(&ctx, state);
}
