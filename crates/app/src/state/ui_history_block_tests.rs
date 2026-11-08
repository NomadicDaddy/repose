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
	// 1.5 pixels per point is a 150% Windows desktop at repose's default zoom. There a 0.03 pt
	// gap crossed a pixel boundary and showed as 1 px in the 1.1.1 round-2 captures, so the
	// baselines must match at each zoom, not merely come within a point.
	for zoom in [1.0, 1.5, 2.25] {
		let ctx = egui::Context::default();
		crate::theme::apply(&ctx, repose_core::config::Theme::Dark);
		ctx.set_zoom_factor(zoom);
		let mut state = expanded(Some(Ok(vec![CommitFile {
			path: "src/new.rs".into(),
			kind: ChangeKind::Renamed,
			from: Some("src/old.rs".into()),
		}])));
		// The zoom takes effect on the next pass.
		frame(&ctx, &mut state, Vec::new());
		let shapes = frame(&ctx, &mut state, Vec::new());
		let letter = baseline_of(&shapes, "R").unwrap_or_else(|| panic!("no letter"));
		let path =
			baseline_of(&shapes, "src/new.rs  ←  src/old.rs").unwrap_or_else(|| panic!("no path"));
		assert!(
			(letter - path).abs() <= 0.001,
			"zoom {zoom}: letter baseline {letter} against path baseline {path}"
		);
	}
}

/// Morticia L-4 on d2b5f6e: with the letter in the proportional path font, M, A, D and R differ in
/// width, so every file's path must still start in one column.
#[test]
fn the_paths_of_an_expanded_commit_start_in_one_column() {
	let ctx = egui::Context::default();
	crate::theme::apply(&ctx, repose_core::config::Theme::Dark);
	let file = |path: &str, kind| CommitFile {
		path: path.into(),
		kind,
		from: None,
	};
	let mut state = expanded(Some(Ok(vec![
		file("src/m.rs", ChangeKind::Modified),
		file("src/a.rs", ChangeKind::Added),
		file("src/d.rs", ChangeKind::Deleted),
		file("src/t.rs", ChangeKind::TypeChanged),
	])));
	let shapes = frame(&ctx, &mut state, Vec::new());
	let xs: Vec<(String, f32)> = shapes
		.iter()
		.filter_map(|s| match &s.shape {
			egui::epaint::Shape::Text(t) if t.galley.text().starts_with("src/") => {
				Some((t.galley.text().to_string(), t.pos.x))
			}
			_ => None,
		})
		.collect();
	assert_eq!(xs.len(), 4, "four paths: {xs:?}");
	let first = xs[0].1;
	assert!(
		xs.iter().all(|(_, x)| (x - first).abs() <= 0.5),
		"the paths start in one column: {xs:?}"
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
