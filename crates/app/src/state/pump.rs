//! Draining what the background threads sent into the model, once a frame.

use eframe::egui;

use super::{AppState, FileRev, Msg, OpenDiff, OpenFile};

impl AppState {
	/// Drains arrived messages into the model. Stale generations are dropped.
	pub fn pump(&mut self, ctx: &egui::Context) {
		while let Ok(msg) = self.rx.try_recv() {
			match msg {
				Msg::Discovered(g, repos) if g == self.generation => {
					let landed = self.merge_discovered(repos);
					self.sync_watch(ctx);
					// A --file diff opens only on the repository that was asked for; a --select
					// that matched nothing falls back to the first row, and that row's working
					// tree is not the one the caller meant.
					if let Some((path, inline)) = self.requested_diff.take()
						&& landed && let Some(repo) = self.selected.clone()
					{
						self.open_diff_in(
							OpenDiff {
								repo,
								source: repose_core::model::DiffSource::Working,
								paths: vec![path.clone()],
								display: path,
							},
							inline,
							ctx,
						);
					}
					if let Some(path) = self.requested_file.take()
						&& landed && let Some(repo) = self.selected.clone()
					{
						self.open_file(
							OpenFile {
								repo,
								rev: FileRev::Head,
								path,
								size: None,
							},
							ctx,
						);
					}
				}
				Msg::Status(g, repo) if g == self.generation => {
					let path = repo.path.clone();
					let merged = self.merge_status(*repo);
					if merged.head_changed {
						self.refresh_history(&path, ctx);
						self.refresh_files_for(&path, ctx);
					}
					// Every accepted status read re-reads a loaded branch list: a branch made or
					// deleted in a terminal changes nothing on the row, yet the watcher fires on
					// refs/heads and Refresh re-reads every row, and this is where both land
					// (Roger on ef766e2). One for-each-ref per status read, and only for
					// repositories whose tab has been opened.
					if merged.applied {
						self.refresh_branches(&path, ctx);
					}
					if merged.applied {
						self.reload_diff_after_status(&path, merged.changed, ctx);
					}
				}
				Msg::Done(g, offset) if g == self.generation => {
					self.scanning = false;
					self.pass_done(offset, ctx);
				}
				Msg::PassDone(g, offset) if g == self.generation => self.pass_done(offset, ctx),
				Msg::Notice(text) => self.notice = Some(text),
				Msg::WriteDone(outcome) => self.finish_write(*outcome, ctx),
				Msg::History(h) => self.merge_history(h),
				Msg::Branches(m) => self.merge_branches(m),
				Msg::Diff(m) => self.merge_diff(m),
				Msg::Files(m) => self.merge_files(m),
				Msg::Remote(g, repo, remote) => self.merge_remote(g, repo, remote),
				_ => {}
			}
		}
	}
}
