//! An expanded commit in History (screens/history_block.rs): the status line while its files are
//! read, unreadable or empty, and a click on a renamed file opening that commit's diff with both
//! names, drawn through the real frame.

use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;
use repose_core::model::{ChangeKind, Commit, CommitFile, DiffSource};

use super::ui_harness::{click, frame, repo, text_at, texts};
use super::{AppState, HistoryState, View};

const HASH: &str = "b1b2b3b4b5b6b7b8b9b0b1b2b3b4b5b6b7b8b9b0";

/// A selected repository on History with one expanded commit whose files are `files` (None
/// leaves them unread).
fn expanded(files: Option<Result<Vec<CommitFile>, String>>) -> AppState {
	let mut state = AppState::new(Config::default());
	let row = repo("history-block", SystemTime::now());
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	state.view = View::History;
	let mut history = HistoryState {
		commits: vec![Commit {
			hash: HASH.into(),
			short: HASH[..7].into(),
			author: "Ada".into(),
			time: 0,
			date: "1970-01-01T00:00:00Z".into(),
			subject: "Move the module".into(),
			body: String::new(),
		}],
		exhausted: true,
		..HistoryState::default()
	};
	history.expanded = Some(HASH.into());
	if let Some(files) = files {
		history.files.insert(HASH.into(), files);
	}
	state.history.insert(path, history);
	state
}

#[test]
fn the_expanded_commit_says_why_it_lists_no_files() {
	let cases = [
		(
			Some(Err("bad object".to_string())),
			"Files could not be read: bad object",
		),
		(Some(Ok(Vec::new())), "No files changed"),
		(None, "Reading files…"),
	];
	for (files, line) in cases {
		let ctx = egui::Context::default();
		let mut state = expanded(files);
		let shown = texts(&frame(&ctx, &mut state, Vec::new()));
		assert!(shown.iter().any(|t| t == line), "{line:?} not in {shown:?}");
	}
}

/// The absolute baseline of the painted text equal to `label`: its galley position plus its
/// first row's offset plus that row's first glyph's baseline.
fn baseline_of(shapes: &[egui::epaint::ClippedShape], label: &str) -> Option<f32> {
	shapes.iter().find_map(|s| match &s.shape {
		egui::epaint::Shape::Text(t) if t.galley.text() == label => {
			let row = t.galley.rows.first()?;
			let glyph = row.row.glyphs.first()?;
			Some(t.pos.y + row.pos.y + glyph.pos.y)
		}
		_ => None,
	})
}

/// Design-polish clause 6: in an expanded commit, the change letter sits on the path's baseline.
#[test]
fn the_change_letter_sits_on_the_paths_baseline() {
	let ctx = egui::Context::default();
	crate::theme::apply(&ctx);
	let mut state = expanded(Some(Ok(vec![CommitFile {
		path: "src/new.rs".into(),
		kind: ChangeKind::Renamed,
		from: Some("src/old.rs".into()),
	}])));
	let shapes = frame(&ctx, &mut state, Vec::new());
	let letter = baseline_of(&shapes, "R").unwrap_or_else(|| panic!("no letter"));
	let path =
		baseline_of(&shapes, "src/new.rs  ←  src/old.rs").unwrap_or_else(|| panic!("no path"));
	// Within half a point: the 1.1.1 captures at 150% showed a 1 px tolerance still let the
	// letter sit about 2 screen pixels high when it was monospace and the path proportional.
	assert!(
		(letter - path).abs() <= 0.5,
		"letter baseline {letter} against path baseline {path}"
	);
}

#[test]
fn clicking_a_renamed_file_opens_the_commits_diff_with_both_names() {
	let ctx = egui::Context::default();
	let mut state = expanded(Some(Ok(vec![CommitFile {
		path: "src/new.rs".into(),
		kind: ChangeKind::Renamed,
		from: Some("src/old.rs".into()),
	}])));
	let shapes = frame(&ctx, &mut state, Vec::new());
	let shown = texts(&shapes);
	assert!(shown.iter().any(|t| t == "R"), "the letter: {shown:?}");
	let Some(at) = text_at(&shapes, "src/new.rs  ←  src/old.rs") else {
		panic!("no renamed row in {shown:?}");
	};
	assert!(state.diff.is_none(), "nothing open before the click");
	frame(&ctx, &mut state, click(at));
	let Some(diff) = &state.diff else {
		panic!("the click opened no diff");
	};
	assert_eq!(diff.open.source, DiffSource::Commit(HASH.into()));
	assert_eq!(diff.open.paths, ["src/new.rs", "src/old.rs"]);
	assert_eq!(diff.open.display, "src/new.rs");
}
