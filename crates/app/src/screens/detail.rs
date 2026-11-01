//! The right pane. Until the Changes, History and diff records land it shows the selected
//! repository's name, branch line, path and status counts.

use eframe::egui::{self, FontFamily, RichText, Ui};

use crate::state::AppState;
use crate::theme::*;

pub fn show(root: &mut Ui, state: &AppState) {
	egui::CentralPanel::default()
		.frame(egui::Frame::new().fill(BG_APP).inner_margin(16.0))
		.show(root, |ui| {
			let Some(repo) = state.selected_repo() else {
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
			ui.add_space(12.0);
			match (&repo.status, &repo.error) {
				(_, Some(e)) => {
					ui.label(RichText::new("Status unavailable").color(DANGER));
					ui.label(RichText::new(e).size(TEXT_SMALL).color(MUTED));
				}
				(Some(s), None) => {
					ui.label(
						RichText::new(format!(
							"{} staged · {} changed · {} untracked · {} conflicts",
							s.staged, s.unstaged, s.untracked, s.conflicts
						))
						.color(TEXT),
					);
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
				}
			}
		});
}
