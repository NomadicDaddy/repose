//! Fixtures and frame helpers for the UI tests: a headless platform, the shared frame and its
//! input events (from ui_frame.rs), scroll offsets read back through the published ids, the
//! row rectangles a frame drew and the one drawn with a given fill and outline, and repository
//! and diff fixtures.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use eframe::egui::epaint::{ClippedShape, Shape};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2, vec2};
use repose_core::git::parse::parse_unified;
use repose_core::model::{
	BranchState, ChangeKind, Changes, FileChange, FileDiff, RepoKind, RepoSummary, StatusSummary,
};

use super::{AppState, DiffContent, DiffMsg, Msg};
use crate::diff_rows::build_rows;
use crate::platform::Platform;
use crate::theme::{ACCENT, ROW_HEIGHT, SIDEBAR_WIDTH};

/// A platform that opens nothing; the frames here never click an action.
pub(super) struct Headless;

impl Platform for Headless {
	fn open_folder(&self, _dir: &Path) -> Result<(), String> {
		Err("headless".into())
	}
	fn reveal_file(&self, _file: &Path) -> Result<(), String> {
		Err("headless".into())
	}
	fn open_in_editor(&self, _c: &str, _t: &Path, _l: Option<usize>) -> Result<(), String> {
		Err("headless".into())
	}
	fn open_terminal(&self, _c: &str, _dir: &Path) -> Result<(), String> {
		Err("headless".into())
	}
}

pub(super) const SCREEN: Vec2 = vec2(1200.0, 400.0);

// The frame and the input events it carries live in ui_frame.rs, the one place a test frame is
// composed.
pub(super) use super::ui_frame::{click, frame, frame_on, key, settle, tab, wheel};

/// Every text the frame painted, in paint order, for a test of what a row says, not where.
pub(super) fn texts(shapes: &[ClippedShape]) -> Vec<String> {
	shapes
		.iter()
		.filter_map(|s| match &s.shape {
			Shape::Text(t) => Some(t.galley.text().to_string()),
			_ => None,
		})
		.collect()
}

/// The centre of where the frame painted `label`, for a click on it.
pub(super) fn text_at(shapes: &[ClippedShape], label: &str) -> Option<Pos2> {
	fn walk(shape: &Shape, label: &str) -> Option<Pos2> {
		match shape {
			Shape::Vec(inner) => inner.iter().find_map(|s| walk(s, label)),
			Shape::Text(t) if t.galley.text() == label => {
				Some(t.pos + t.galley.rect.center().to_vec2())
			}
			_ => None,
		}
	}
	shapes.iter().find_map(|c| walk(&c.shape, label))
}

/// Every rectangle the frame drew, with its fill and stroke.
fn all_rects(shapes: &[ClippedShape]) -> Vec<(Rect, Color32, Stroke)> {
	fn walk(shape: &Shape, out: &mut Vec<(Rect, Color32, Stroke)>) {
		match shape {
			Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
			Shape::Rect(r) => out.push((r.rect, r.fill, r.stroke)),
			_ => {}
		}
	}
	let mut out = Vec::new();
	for clipped in shapes {
		walk(&clipped.shape, &mut out);
	}
	out
}

/// Every rectangle the frame drew inside the sidebar, with its fill and stroke.
fn sidebar_rects(shapes: &[ClippedShape]) -> Vec<(Rect, Color32, Stroke)> {
	all_rects(shapes)
		.into_iter()
		.filter(|(r, _, _)| r.right() <= SIDEBAR_WIDTH)
		.collect()
}

/// The rectangles of one row height drawn in the right pane: the change, commit or file-view
/// line backgrounds and outlines, whatever state each was in.
pub(super) fn pane_rects(shapes: &[ClippedShape], height: f32) -> Vec<(Rect, Color32, Stroke)> {
	all_rects(shapes)
		.into_iter()
		.filter(|(r, _, _)| r.left() >= SIDEBAR_WIDTH && (r.height() - height).abs() < 0.5)
		.collect()
}

/// The sidebar rectangles at a repository row's height: the row backgrounds and outlines,
/// whatever state each row was in.
pub(super) fn row_rects(shapes: &[ClippedShape]) -> Vec<(Rect, Color32, Stroke)> {
	sidebar_rects(shapes)
		.into_iter()
		.filter(|(r, _, _)| (r.height() - ROW_HEIGHT).abs() < 0.5)
		.collect()
}

/// Every one-pixel bright accent outline the frame drew inside the sidebar, whatever its size:
/// where keyboard focus is, on a row or on a worktree chevron.
pub(super) fn accent_outlines(shapes: &[ClippedShape]) -> Vec<Rect> {
	sidebar_rects(shapes)
		.into_iter()
		.filter(|(_, _, s)| s.color == ACCENT.get() && s.width == 1.0)
		.map(|(r, _, _)| r)
		.collect()
}

/// The first of `rects` filled with `fill` under a one-pixel `line`, or with no outline when
/// `line` is transparent: the selected, focused or hovered row of a frame.
pub(super) fn rect_with(
	rects: &[(Rect, Color32, Stroke)],
	fill: impl Into<Color32>,
	line: impl Into<Color32>,
) -> Option<Rect> {
	let (fill, line) = (fill.into(), line.into());
	rects
		.iter()
		.find(|(_, f, s)| {
			*f == fill && s.color == line && (s.width == 1.0 || line == Color32::TRANSPARENT)
		})
		.map(|(r, _, _)| *r)
}

/// The clamped offset of the scroll area whose id was published under `key` on the last frame.
pub(super) fn offset(ctx: &egui::Context, key: egui::Id) -> Vec2 {
	let id: egui::Id = ctx
		.data(|d| d.get_temp(key))
		.unwrap_or_else(|| panic!("no scroll id published under {key:?}"));
	egui::scroll_area::State::load(ctx, id)
		.unwrap_or_else(|| panic!("no scroll state for {id:?}"))
		.clamped_offset()
}

/// A repository row on main with one modified file, under a path that holds no repository.
pub(super) fn repo(name: &str, checked_at: SystemTime) -> RepoSummary {
	repo_at(
		std::env::temp_dir().join("repose-ui").join(name),
		checked_at,
	)
}

/// [`repo`] with three unstaged modified files, a.txt, b.txt and c.txt, merged into `state`.
/// Returns its path.
pub(super) fn with_three_changes(state: &mut AppState, name: &str) -> PathBuf {
	let mut row = repo(name, SystemTime::now());
	if let Some(changes) = row.changes.as_mut() {
		changes.unstaged = ["a.txt", "b.txt", "c.txt"]
			.into_iter()
			.map(|p| FileChange {
				path: p.into(),
				kind: ChangeKind::Modified,
				from: None,
			})
			.collect();
	}
	if let Some(status) = row.status.as_mut() {
		status.unstaged = 3;
	}
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	path
}

/// A real, empty-diff repository on disk for a row whose diff the app will really re-read:
/// `git init`, one committed `x.txt`, nothing changed since, so `git diff -- x.txt` is empty and
/// a reader given the hash of empty bytes as the known hash sends nothing back.
pub(super) fn real_repo(name: &str) -> PathBuf {
	let dir = std::env::temp_dir().join("repose-ui-real").join(name);
	let _ = std::fs::remove_dir_all(&dir);
	std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
	std::fs::write(dir.join("x.txt"), "one\n").unwrap_or_else(|e| panic!("{e}"));
	for args in [
		vec!["init", "-q"],
		vec!["add", "--", "x.txt"],
		vec![
			"-c",
			"user.name=repose test",
			"-c",
			"user.email=repose@example.invalid",
			"commit",
			"-q",
			"-m",
			"init",
		],
	] {
		let out = Command::new("git")
			.args(&args)
			.current_dir(&dir)
			.output()
			.unwrap_or_else(|e| panic!("git {}: {e}", args.join(" ")));
		assert!(
			out.status.success(),
			"git {} failed: {}",
			args.join(" "),
			String::from_utf8_lossy(&out.stderr)
		);
	}
	dir
}

/// A repository row on main with one modified file, at the given path.
pub(super) fn repo_at(path: PathBuf, checked_at: SystemTime) -> RepoSummary {
	let mut r = RepoSummary::new(path, RepoKind::Repository);
	r.branch = BranchState::Branch("main".into());
	r.short_hash = Some("0123abc".into());
	r.status = Some(StatusSummary {
		unstaged: 1,
		..StatusSummary::default()
	});
	r.changes = Some(Changes {
		unstaged: vec![FileChange {
			path: "x.txt".into(),
			kind: ChangeKind::Modified,
			from: None,
		}],
		..Changes::default()
	});
	r.checked_at = Some(checked_at);
	r
}

/// A one-file diff of `lines` added lines; 200 is long enough to scroll inside a 400 px window.
pub(super) fn diff_of(lines: usize) -> Vec<FileDiff> {
	let mut text =
		format!("diff --git a/x.txt b/x.txt\n--- a/x.txt\n+++ b/x.txt\n@@ -1,0 +1,{lines} @@\n");
	for i in 0..lines {
		text.push_str(&format!("+line {i}\n"));
	}
	parse_unified(text.as_bytes())
}

/// What the reader thread sends when a re-read of the open diff lands with different bytes.
pub(super) fn landed(generation: u64, lines: usize, raw_hash: u64) -> Msg {
	let files = diff_of(lines);
	let rows = vec![build_rows(&files[0], false)];
	Msg::Diff(DiffMsg {
		generation,
		content: DiffContent::Files(files),
		rows,
		rows_inline: false,
		colours: Vec::new(),
		raw_hash: Some(raw_hash),
	})
}
