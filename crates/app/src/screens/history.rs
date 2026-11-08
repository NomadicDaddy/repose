//! The History pane: the current branch newest first, an Outgoing changes group for commits not
//! on the upstream, each commit expanding to its message and files, and Load more.

use eframe::egui::{self, Align2, FontFamily, FontId, RichText, Sense, Ui, pos2, vec2};
use repose_core::model::{BranchState, Commit, RepoSummary, relative};

use super::row::{fit, frame};
use super::{a11y, fetch, menu};
use crate::lanes::GraphRow;
use crate::platform::Platform;
use crate::state::AppState;
use crate::theme::*;

#[path = "history_block.rs"]
mod block;
#[path = "history_lanes.rs"]
mod lanes;
use block::expanded_block;

pub fn show(ui: &mut Ui, state: &mut AppState, platform: &dyn Platform) {
	let Some(repo) = state.selected_repo().cloned() else {
		return;
	};
	// The History header: Fetch, and what the counts below are read from.
	ui.horizontal(|ui| {
		fetch::button(ui, state, &repo.path);
		let note = fetch::refs_note(state, &repo.path);
		ui.label(RichText::new(note).size(TEXT_SMALL).color(MUTED));
		// Off by default; on draws every ref's lanes and keeps that for the session (clause 2).
		let mut on = state.lanes;
		let toggle = ui
			.toggle_value(&mut on, "Lanes")
			.on_hover_text("Draw the lanes of every branch, with their merges and forks");
		if toggle.changed() {
			state.set_lanes(on, ui.ctx());
		}
	});
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
	// The graph applies only while its rows match the commits on screen: after the toggle the
	// old scope's rows stand, without a graph, until the new first page lands.
	let graph = if state.lanes && h.graph.len() == commits.len() {
		h.graph.clone()
	} else {
		Vec::new()
	};
	let rows = Rows {
		expanded: expanded.as_deref(),
		files: &files,
		now,
		platform,
		graph_width: lanes::graph_width(&graph),
	};
	if !graph.is_empty() {
		// Every ref in topological order, one list: splitting out the outgoing commits would
		// break the lanes.
		for (c, lane) in commits.iter().zip(&graph) {
			commit_row(ui, state, &repo, c, &rows, Some(lane));
		}
	} else {
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
				commit_row(ui, state, &repo, c, &rows, None);
			}
			ui.add_space(8.0);
			section(ui, "Earlier", rest.len());
		}
		for c in rest {
			commit_row(ui, state, &repo, c, &rows, None);
		}
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
	/// The room the lanes take at the left of every row; zero in the linear view.
	graph_width: f32,
}

fn commit_row(
	ui: &mut Ui,
	state: &mut AppState,
	repo: &RepoSummary,
	c: &Commit,
	rows: &Rows<'_>,
	lane: Option<&GraphRow>,
) {
	let (expanded, files, now) = (rows.expanded, rows.files, rows.now);
	let open = expanded == Some(c.hash.as_str());
	let (rect, response) = ui.allocate_exact_size(
		vec2(ui.available_width(), COMMIT_ROW_HEIGHT),
		Sense::click(),
	);
	let painter = ui.painter_at(rect);
	let fill = if open {
		Some(ACCENT_BG)
	} else if response.hovered() {
		Some(BG_HOVER)
	} else {
		None
	};
	let line = open.then_some(ACCENT_LINE);
	frame(&painter, rect, RADIUS, fill, line, response.has_focus());
	if let Some(l) = lane {
		let gap = ui.spacing().item_spacing.y;
		lanes::draw(ui.painter(), rect, gap, l, c.parents.len() > 1);
	}
	let x = rect.left() + 8.0 + rows.graph_width;
	painter.text(
		pos2(x, rect.center().y),
		Align2::LEFT_CENTER,
		&c.short,
		FontId::new(TEXT_MONO, FontFamily::Monospace),
		INFO.get(),
	);
	let meta = format!("{} · {}", c.author, relative(c.time, now, &c.date));
	let meta_galley = painter.layout_no_wrap(
		meta,
		FontId::new(TEXT_SMALL, FontFamily::Proportional),
		MUTED.get(),
	);
	let meta_w = meta_galley.size().x;
	painter.galley(
		pos2(
			rect.right() - 10.0 - meta_w,
			rect.center().y - meta_galley.size().y / 2.0,
		),
		meta_galley,
		MUTED.get(),
	);
	let subject_left = x + 70.0;
	let mut subject_right = rect.right() - 20.0 - meta_w;
	if lane.is_some() {
		subject_right = lanes::refs(&painter, c, subject_right, rect.center().y);
	}
	let subject_width = (subject_right - subject_left).max(40.0);
	// One line, cut with an ellipsis when it is wider than the room beside the hash and the meta:
	// laid out with wrapping and clipped, a long subject showed the top of its second line inside
	// the row (F4 sweep on 6445c56). A cut subject is shown whole in the tooltip.
	let font = FontId::new(TEXT_BASE, FontFamily::Proportional);
	let subject = fit(&painter, &c.subject, font, TEXT, subject_width);
	let cut = subject.text() != c.subject;
	let subject_y = rect.center().y - subject.size().y / 2.0;
	painter.galley(pos2(subject_left, subject_y), subject, TEXT.get());
	let tooltip = if cut {
		format!("{}\n{}", c.subject, iso_tooltip(c))
	} else {
		iso_tooltip(c)
	};
	let response = response.on_hover_text(tooltip);
	let name = a11y::commit_name(c, &relative(c.time, now, &c.date));
	a11y::control(&response, egui::Role::Button, Some(open), &name);
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
