//! Fixtures and frame helpers for the UI tests: a headless platform, one frame drawn through the
//! real screens that hands back what it painted, wheel and Tab events, scroll offsets read back
//! through the published ids, the row rectangles a frame drew, and repository and diff fixtures.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use eframe::egui::epaint::{ClippedShape, Shape};
use eframe::egui::{self, Color32, Event, MouseWheelUnit, Pos2, Rect, Stroke, Vec2, vec2};
use repose_core::git::parse::parse_unified;
use repose_core::model::{
	BranchState, ChangeKind, Changes, FileChange, FileDiff, RepoKind, RepoSummary, StatusSummary,
};

use super::{AppState, DiffContent, DiffMsg, Msg};
use crate::diff_rows::build_rows;
use crate::platform::Platform;
use crate::screens::{detail, repo_list};
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

/// One frame of the app as main.rs draws it: the left list, then the right pane. Returns what
/// the frame painted, for a test that checks what a state looks like rather than what it is.
pub(super) fn frame(
	ctx: &egui::Context,
	state: &mut AppState,
	events: Vec<Event>,
) -> Vec<ClippedShape> {
	let input = egui::RawInput {
		screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
		events,
		..Default::default()
	};
	let mut out = ctx.run_ui(input, |root| {
		repo_list::show(root, state, &Headless);
		detail::show(root, state, &Headless);
	});
	// No renderer takes the font texture here; epaint insists the deltas are cleared, not dropped.
	out.textures_delta.clear();
	out.shapes
}

/// Every piece of text the frame painted, in paint order, for a test about what a header or a
/// row says rather than where it sits.
pub(super) fn texts(shapes: &[ClippedShape]) -> Vec<String> {
	shapes
		.iter()
		.filter_map(|s| match &s.shape {
			Shape::Text(t) => Some(t.galley.text().to_string()),
			_ => None,
		})
		.collect()
}

/// One press and release of Tab, which moves keyboard focus to the next control.
pub(super) fn tab() -> Vec<Event> {
	key(egui::Key::Tab)
}

/// One press and release of `k`.
pub(super) fn key(k: egui::Key) -> Vec<Event> {
	[true, false]
		.into_iter()
		.map(|pressed| Event::Key {
			key: k,
			physical_key: None,
			pressed,
			repeat: false,
			modifiers: egui::Modifiers::default(),
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

/// A primary click at `at`: move, press, release.
pub(super) fn click(at: Pos2) -> Vec<Event> {
	let button = |pressed| Event::PointerButton {
		pos: at,
		button: egui::PointerButton::Primary,
		pressed,
		modifiers: egui::Modifiers::default(),
	};
	vec![Event::PointerMoved(at), button(true), button(false)]
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
		.filter(|(_, _, s)| s.color == ACCENT && s.width == 1.0)
		.map(|(r, _, _)| r)
		.collect()
}

/// Enough quiet frames (1/60 s each) for a scroll animation to finish.
pub(super) fn settle(ctx: &egui::Context, state: &mut AppState) {
	for _ in 0..40 {
		frame(ctx, state, Vec::new());
	}
}

/// A wheel turn with the pointer over `at`, enough to move a long list or diff.
pub(super) fn wheel(at: Pos2) -> Vec<Event> {
	vec![
		Event::PointerMoved(at),
		Event::MouseWheel {
			unit: MouseWheelUnit::Point,
			delta: vec2(0.0, -240.0),
			phase: egui::TouchPhase::Move,
			source: egui::MouseWheelSource::Wheel,
			modifiers: egui::Modifiers::default(),
		},
	]
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
