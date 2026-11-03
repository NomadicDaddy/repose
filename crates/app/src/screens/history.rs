//! The History pane: the current branch newest first, an Outgoing changes group for commits not
//! on the upstream, each commit expanding to its message and files, and Load more.

use eframe::egui::{self, Align2, FontFamily, FontId, RichText, Sense, StrokeKind, Ui, pos2, vec2};
use repose_core::model::{BranchState, ChangeKind, Commit, DiffSource, RepoSummary, relative};

use super::menu;
use crate::platform::Platform;
use crate::state::{AppState, OpenDiff};
use crate::theme::*;

pub fn show(ui: &mut Ui, state: &mut AppState, platform: &dyn Platform) {
	let Some(repo) = state.selected_repo().cloned() else {
		return;
	};
	if repo.branch == BranchState::Unborn {
		ui.label(RichText::new("No commits yet").color(MUTED));
		return;
	}
	let ctx = ui.ctx().clone();
	state.ensure_history(&repo.path, &ctx);
	let Some(h) = state.history_of(&repo.path) else {
		return;
	};
	let commits = h.commits.clone();
	let outgoing_set = h.outgoing.clone();
	let expanded = h.expanded.clone();
	let files = h.files.clone();
	let loading = h.loading;
	let exhausted = h.exhausted;
	let error = h.error.clone().or_else(|| h.outgoing_error.clone());
	if let Some(e) = error {
		ui.label(RichText::new(e).size(TEXT_SMALL).color(DANGER));
	}
	let now = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs() as i64)
		.unwrap_or(0);
	// Membership, not position: after a merge the unpushed commits are interleaved with the
	// upstream's in date order, so the set from <upstream>..HEAD decides.
	let (outgoing, rest): (Vec<&Commit>, Vec<&Commit>) =
		commits.iter().partition(|c| outgoing_set.contains(&c.hash));
	if !outgoing.is_empty() {
		let branch = match &repo.branch {
			BranchState::Branch(b) => b.clone(),
			_ => String::new(),
		};
		section(ui, &format!("Outgoing changes · {branch}"), outgoing.len());
		for c in outgoing {
			commit_row(
				ui,
				state,
				&repo,
				c,
				&Rows {
					expanded: expanded.as_deref(),
					files: &files,
					now,
					platform,
				},
			);
		}
		ui.add_space(8.0);
		section(ui, "Earlier", rest.len());
	}
	for c in rest {
		commit_row(
			ui,
			state,
			&repo,
			c,
			&Rows {
				expanded: expanded.as_deref(),
				files: &files,
				now,
				platform,
			},
		);
	}
	if commits.is_empty() && loading {
		ui.label(RichText::new("Reading history…").color(MUTED));
	}
	if !exhausted {
		ui.add_space(8.0);
		let label = if loading { "Loading…" } else { "Load more" };
		if ui.add_enabled(!loading, egui::Button::new(label)).clicked() {
			state.load_more_history(&repo.path, &ctx);
		}
	}
}

fn section(ui: &mut Ui, title: &str, count: usize) {
	ui.add_space(6.0);
	ui.horizontal(|ui| {
		ui.label(
			RichText::new(title.to_uppercase())
				.size(TEXT_SMALL)
				.color(ACCENT)
				.strong(),
		);
		ui.label(
			RichText::new(count.to_string())
				.size(TEXT_SMALL)
				.color(MUTED),
		);
	});
}

/// What every commit row shares: which commit is open, the files fetched so far, and the clock.
struct Rows<'a> {
	expanded: Option<&'a str>,
	files:
		&'a std::collections::HashMap<String, Result<Vec<repose_core::model::CommitFile>, String>>,
	now: i64,
	platform: &'a dyn Platform,
}

fn commit_row(ui: &mut Ui, state: &mut AppState, repo: &RepoSummary, c: &Commit, rows: &Rows<'_>) {
	let (expanded, files, now) = (rows.expanded, rows.files, rows.now);
	let open = expanded == Some(c.hash.as_str());
	let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
	let painter = ui.painter_at(rect);
	if open {
		painter.rect(
			rect,
			RADIUS,
			ACCENT_BG,
			egui::Stroke::new(1.0, ACCENT_LINE),
			StrokeKind::Inside,
		);
	} else if response.hovered() {
		painter.rect_filled(rect, RADIUS, BG_HOVER);
	}
	let x = rect.left() + 8.0;
	painter.text(
		pos2(x, rect.center().y),
		Align2::LEFT_CENTER,
		&c.short,
		FontId::new(12.0, FontFamily::Monospace),
		INFO,
	);
	let meta = format!("{} · {}", c.author, relative(c.time, now, &c.date));
	let meta_galley = painter.layout_no_wrap(
		meta,
		FontId::new(TEXT_SMALL, FontFamily::Proportional),
		MUTED,
	);
	let meta_w = meta_galley.size().x;
	painter.galley(
		pos2(
			rect.right() - 10.0 - meta_w,
			rect.center().y - meta_galley.size().y / 2.0,
		),
		meta_galley,
		MUTED,
	);
	let subject_left = x + 70.0;
	let subject_width = (rect.right() - 20.0 - meta_w - subject_left).max(40.0);
	let subject = painter.layout(
		c.subject.clone(),
		FontId::new(TEXT_BASE, FontFamily::Proportional),
		TEXT,
		subject_width,
	);
	let clipped = painter.with_clip_rect(egui::Rect::from_min_size(
		pos2(subject_left, rect.top()),
		vec2(subject_width, rect.height()),
	));
	clipped.galley(pos2(subject_left, rect.center().y - 9.0), subject, TEXT);
	let response = response.on_hover_text(iso_tooltip(c));
	let ctx = ui.ctx().clone();
	if response.clicked() {
		state.toggle_commit(&repo.path, &c.hash, &ctx);
	}
	let mut action = None;
	menu::row_menu(ui, &response, open, |ui| {
		action = menu::commit_items(ui, &c.hash, &c.subject);
	});
	if let Some(a) = action {
		menu::perform(a, state, rows.platform, &ctx);
	}
	if open {
		expanded_block(
			ui,
			state,
			repo,
			c,
			files
				.get(&c.hash)
				.map(|r| r.as_ref().map(Vec::as_slice).map_err(String::as_str)),
			rows.platform,
		);
	}
}

fn iso_tooltip(c: &Commit) -> String {
	format!("{}\n{}\n{}", c.date, c.author, c.hash)
}

fn expanded_block(
	ui: &mut Ui,
	state: &mut AppState,
	repo: &RepoSummary,
	c: &Commit,
	files: Option<Result<&[repose_core::model::CommitFile], &str>>,
	platform: &dyn Platform,
) {
	let mut action = None;
	egui::Frame::new()
		.fill(BG_PANEL)
		.inner_margin(10.0)
		.corner_radius(RADIUS)
		.show(ui, |ui| {
			if !c.body.is_empty() {
				ui.label(RichText::new(&c.body).size(TEXT_BASE).color(TEXT));
				ui.add_space(6.0);
			}
			match files {
				None => {
					ui.label(
						RichText::new("Reading files…")
							.size(TEXT_SMALL)
							.color(MUTED),
					);
				}
				Some(Err(e)) => {
					ui.label(
						RichText::new(format!("Files could not be read: {e}"))
							.size(TEXT_SMALL)
							.color(DANGER),
					);
				}
				Some(Ok([])) => {
					ui.label(
						RichText::new("No files changed")
							.size(TEXT_SMALL)
							.color(MUTED),
					);
				}
				Some(Ok(list)) => {
					// The working copy matches only this commit's files when it is HEAD; a file
					// deleted here has none (Morticia L-2 on 85ec3b0).
					let at_head = repo
						.short_hash
						.as_deref()
						.is_some_and(|s| c.hash.starts_with(s));
					for f in list {
						let absent = if f.kind == ChangeKind::Deleted {
							Some("Deleted in this commit; there is no working copy")
						} else if !at_head {
							Some("Changed at an older commit; the working copy may differ. Open it from Changes or the Files tab at HEAD.")
						} else {
							None
						};
						ui.horizontal(|ui| {
							ui.label(
								RichText::new(f.kind.letter())
									.family(FontFamily::Monospace)
									.color(letter_color(f.kind)),
							);
							let text = match &f.from {
								Some(from) => format!("{}  ←  {from}", f.path),
								None => f.path.clone(),
							};
							let row = ui.selectable_label(
								false,
								RichText::new(text).size(TEXT_BASE).color(TEXT),
							);
							menu::row_menu(ui, &row, false, |ui| {
								action = menu::file_items(ui, &repo.path, &f.path, None, absent);
							});
							if row.clicked() {
								let mut paths = vec![f.path.clone()];
								if let Some(from) = &f.from {
									paths.push(from.clone());
								}
								let ctx = ui.ctx().clone();
								state.open_diff(
									OpenDiff {
										repo: repo.path.clone(),
										source: DiffSource::Commit(c.hash.clone()),
										paths,
										display: f.path.clone(),
									},
									&ctx,
								);
							}
						});
					}
				}
			}
		});
	if let Some(a) = action {
		let ctx = ui.ctx().clone();
		menu::perform(a, state, platform, &ctx);
	}
}

fn letter_color(kind: ChangeKind) -> egui::Color32 {
	match kind {
		ChangeKind::Modified | ChangeKind::TypeChanged => WARN,
		ChangeKind::Added | ChangeKind::IntentToAdd => ACCENT,
		ChangeKind::Deleted | ChangeKind::Conflicted => DANGER,
		ChangeKind::Renamed => INFO,
		ChangeKind::Untracked => MUTED,
	}
}
