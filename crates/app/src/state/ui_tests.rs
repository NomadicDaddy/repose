//! Whole frames drawn through the real screens on an egui context, with no window. The scroll
//! offsets are read back through the ids the two panes publish each frame.

use std::path::Path;
use std::time::{Duration, SystemTime};

use eframe::egui::{self, Event, MouseWheelUnit, Pos2, Rect, Vec2, pos2, vec2};
use repose_core::config::Config;
use repose_core::git::parse::parse_unified;
use repose_core::model::{
	BranchState, ChangeKind, Changes, DiffSource, FileChange, RepoKind, RepoSummary, StatusSummary,
};

use super::{AppState, DiffContent, DiffState, Msg, OpenDiff};
use crate::diff_rows::build_rows;
use crate::platform::Platform;
use crate::screens::{detail, diff, repo_list};

/// A platform that opens nothing; the frames here never click an action.
struct Headless;

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

const SCREEN: Vec2 = vec2(1200.0, 400.0);

/// One frame of the app as main.rs draws it: the left list, then the right pane.
fn frame(ctx: &egui::Context, state: &mut AppState, events: Vec<Event>) {
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
}

/// Enough quiet frames (1/60 s each) for a scroll animation to finish.
fn settle(ctx: &egui::Context, state: &mut AppState) {
	for _ in 0..40 {
		frame(ctx, state, Vec::new());
	}
}

/// A wheel turn with the pointer over `at`, enough to move a long list or diff.
fn wheel(at: Pos2) -> Vec<Event> {
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
fn offset(ctx: &egui::Context, key: egui::Id) -> Vec2 {
	let id: egui::Id = ctx
		.data(|d| d.get_temp(key))
		.unwrap_or_else(|| panic!("no scroll id published under {key:?}"));
	egui::scroll_area::State::load(ctx, id)
		.unwrap_or_else(|| panic!("no scroll state for {id:?}"))
		.clamped_offset()
}

fn repo(name: &str, checked_at: SystemTime) -> RepoSummary {
	let mut r = RepoSummary::new(
		std::env::temp_dir().join("repose-ui").join(name),
		RepoKind::Repository,
	);
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

/// A one-file diff long enough to scroll inside a 400 px window.
fn long_diff() -> Vec<repose_core::model::FileDiff> {
	let mut text =
		String::from("diff --git a/x.txt b/x.txt\n--- a/x.txt\n+++ b/x.txt\n@@ -1,0 +1,200 @@\n");
	for i in 0..200 {
		text.push_str(&format!("+line {i}\n"));
	}
	parse_unified(text.as_bytes())
}

/// live-refresh clause 5: a status that arrives for the selected repository, through the same
/// message path a watcher refresh uses, leaves the selected repository, the selected file, the
/// open diff and both scroll positions where they were.
#[test]
fn refresh_preserves_selection() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	state.repos = (0..40).map(|i| repo(&format!("r{i:02}"), t0)).collect();
	let chosen = state.repos[7].path.clone();
	state.selected = Some(chosen.clone());
	state.selected_file = Some("x.txt".into());
	let files = long_diff();
	assert_eq!(files.len(), 1, "the fixture parses to one file");
	let rows = vec![build_rows(&files[0], false)];
	let open = OpenDiff {
		repo: chosen.clone(),
		source: DiffSource::Working,
		paths: vec!["x.txt".into()],
		display: "x.txt".into(),
	};
	state.diff = Some(DiffState {
		open: open.clone(),
		content: DiffContent::Files(files),
		inline: false,
		generation: 1,
		rows,
		rows_inline: false,
		colours: Vec::new(),
		raw_hash: Some(1),
	});

	// Lay out, then scroll each pane with the pointer over it. egui animates a wheel scroll over
	// several frames, so each reading waits for the offsets to settle.
	frame(&ctx, &mut state, Vec::new());
	frame(&ctx, &mut state, wheel(pos2(150.0, 250.0)));
	frame(&ctx, &mut state, wheel(pos2(800.0, 300.0)));
	settle(&ctx, &mut state);
	let list_before = offset(&ctx, repo_list::scroll_key());
	let diff_before = offset(&ctx, diff::scroll_key());
	assert!(list_before.y > 0.0, "the list scrolled: {list_before:?}");
	assert!(diff_before.y > 0.0, "the diff scrolled: {diff_before:?}");

	// A refresh of the selected repository: a newer status with a different file list.
	let mut fresh = repo("r07", t0 + Duration::from_secs(1));
	fresh.status = Some(StatusSummary {
		unstaged: 2,
		..StatusSummary::default()
	});
	if let Some(c) = fresh.changes.as_mut() {
		c.unstaged.push(FileChange {
			path: "y.txt".into(),
			kind: ChangeKind::Modified,
			from: None,
		});
	}
	state
		.tx
		.send(Msg::Status(state.generation, Box::new(fresh)))
		.unwrap_or_else(|e| panic!("{e}"));
	state.pump(&ctx);
	settle(&ctx, &mut state);

	assert_eq!(
		state.selected,
		Some(chosen.clone()),
		"the selected repository"
	);
	assert_eq!(
		state.selected_file.as_deref(),
		Some("x.txt"),
		"the selected file"
	);
	let Some(d) = state.diff.as_ref() else {
		panic!("the diff is still open");
	};
	assert_eq!(d.open, open, "the same diff");
	assert!(
		matches!(d.content, DiffContent::Files(_)),
		"its content stands"
	);
	assert_eq!(
		d.rows.len(),
		1,
		"its rows stand while the re-read is in flight"
	);
	assert_eq!(
		offset(&ctx, repo_list::scroll_key()),
		list_before,
		"the list scroll"
	);
	assert_eq!(
		offset(&ctx, diff::scroll_key()),
		diff_before,
		"the diff scroll"
	);
	let unstaged = super::find(&state.repos, &chosen)
		.and_then(|r| r.status.as_ref())
		.map(|s| s.unstaged);
	assert_eq!(unstaged, Some(2), "the status was applied");
}
