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
				// Broken only at separators, so a long URL never wraps inside a folder or host
				// name; still one widget, so a link keeps one click target and tooltip
				// (header-remote-url-wraps-at-separator).
				let font = egui::FontId::new(TEXT_SMALL, FontFamily::Monospace);
				let shown = wrap_at_separators(ui, &line, &font);
				let text = RichText::new(shown)
					.size(TEXT_SMALL)
					.family(FontFamily::Monospace);
				let page = (entry.name == "origin")
					.then(|| https_page(&entry.url))
					.flatten();
				match page {
					Some(page) => {
						let tip = format!("Open {page}");
						let link = ui.add(egui::Link::new(text.color(ACCENT)));
						// A screen reader hears the line whole, not broken where it wraps.
						a11y::control(&link, egui::Role::Link, None, &line);
						if link.on_hover_text(tip).clicked() {
							ui.ctx().open_url(egui::OpenUrl::new_tab(page));
						}
					}
					None => {
						ui.add(
							egui::Label::new(text.color(MUTED))
								.wrap_mode(egui::TextWrapMode::Extend),
						);
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

/// `line` with a line break before each separator-ended piece that would not fit the row's width,
/// so it wraps only after a "/" or "\": never inside a folder or host name. A single piece wider
/// than the pane stays whole on its own line.
fn wrap_at_separators(ui: &Ui, line: &str, font: &egui::FontId) -> String {
	let room = ui.available_width();
	let width = |piece: &str| {
		ui.painter()
			.layout_no_wrap(piece.to_string(), font.clone(), egui::Color32::WHITE)
			.size()
			.x
	};
	let mut out = String::new();
	let mut used = 0.0;
	let mut piece = String::new();
	let mut place = |piece: &str, out: &mut String| {
		let w = width(piece);
		if used > 0.0 && used + w > room {
			out.push('\n');
			used = 0.0;
		}
		out.push_str(piece);
		used += w;
	};
	for c in line.chars() {
		piece.push(c);
		if c == '/' || c == '\\' {
			place(&std::mem::take(&mut piece), &mut out);
		}
	}
	if !piece.is_empty() {
		place(&piece, &mut out);
	}
	out
}

/// The header's path, wrapping only at a separator: one unwrapped piece per folder, each ending in
/// its separator, in a wrapping row with no gaps. A plain label broke inside a folder name
/// (design-polish clause 8). A single folder wider than the pane still overflows on its own line.
fn path_line(ui: &mut Ui, path: &str) {
	// No gaps between pieces, and no widget-row minimum between wrapped lines: without these the
	// lines sat a widget row apart, 26 px for 13 px text (seen at --scale 1.5). The wrapped layout
	// reads its row height when it starts, so the spacing is set in a scope around it.
	ui.scope(|ui| {
		let spacing = ui.spacing_mut();
		spacing.item_spacing = egui::vec2(0.0, 0.0);
		spacing.interact_size.y = 0.0;
		ui.horizontal_wrapped(|ui| {
			let mut piece = String::new();
			for c in path.chars() {
				piece.push(c);
				if c == '\\' || c == '/' {
					path_piece(ui, &std::mem::take(&mut piece));
				}
			}
			if !piece.is_empty() {
				path_piece(ui, &piece);
			}
		});
	});
}

fn path_piece(ui: &mut Ui, piece: &str) {
	ui.add(
		egui::Label::new(
			RichText::new(piece)
				.size(TEXT_SMALL)
				.color(MUTED)
				.family(FontFamily::Monospace),
		)
		.wrap_mode(egui::TextWrapMode::Extend),
	);
}

pub fn show(root: &mut Ui, state: &mut AppState, platform: &dyn Platform) {
	let ctx = root.ctx().clone();
	egui::CentralPanel::default()
		.frame(egui::Frame::new().fill(BG_APP.get()).inner_margin(16.0))
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
			path_line(ui, &repo.path.display().to_string());
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
			let scrolled = egui::ScrollArea::vertical()
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
			// `--measure-lanes` counts only the frames in which this offset moved.
			ui.data_mut(|d| d.insert_temp(offset_key(), scrolled.state.clamped_offset().y));
		});
	confirm::show(&ctx, state);
	close_guard::show(&ctx, state);
}

pub(crate) fn offset_key() -> egui::Id {
	egui::Id::unique("detail-pane-scroll-offset")
}

/// The detail pane's vertical scroll offset as the last frame left it.
pub fn scroll_offset(ctx: &egui::Context) -> Option<f32> {
	ctx.data(|d| d.get_temp(offset_key()))
}
