//! audit-frontend-1793711157 criterion 1: how long one frame of the detail pane takes with a long
//! list in it, against the same view with a short one. Ignored by default; run in release with
//! `cargo test --release -p repose list_frame_times -- --ignored --nocapture`.

use std::time::{Duration, Instant, SystemTime};

use eframe::egui;
use repose_core::config::Config;
use repose_core::model::{ChangeKind, Commit, FileChange, TreeEntry};

use super::ui_harness::{frame, repo};
use super::{AppState, FileRev, FilesState, HistoryState, View};
use crate::file_tree::build_folder_tree;

fn commit(n: usize) -> Commit {
	let hash = format!("{n:040x}");
	Commit {
		short: hash[..7].into(),
		hash,
		author: "Ada".into(),
		time: 0,
		date: "1970-01-01T00:00:00Z".into(),
		subject: format!("Commit number {n} with a subject of ordinary length"),
		..Commit::default()
	}
}

fn state_with(view: View, n: usize) -> AppState {
	let mut state = AppState::new(Config::default());
	let mut row = repo("measure", SystemTime::now());
	let path = row.path.clone();
	if let Some(c) = row.changes.as_mut() {
		c.unstaged = (0..n)
			.map(|i| FileChange {
				path: format!("src/folder{}/file{i:05}.rs", i % 50),
				kind: ChangeKind::Modified,
				from: None,
			})
			.collect();
	}
	state.selected = Some(path.clone());
	state.merge_discovered(vec![row]);
	state.view = view;
	state.history.insert(
		path.clone(),
		HistoryState {
			commits: (0..n).map(commit).collect(),
			exhausted: true,
			..HistoryState::default()
		},
	);
	let entries: Vec<TreeEntry> = (0..n)
		.map(|i| TreeEntry {
			path: format!("file{i:05}.rs"),
			size: 100,
			submodule: false,
		})
		.collect();
	state.files.insert(
		path,
		FilesState {
			rev: FileRev::Head,
			loaded: Some(FileRev::Head),
			tree: build_folder_tree(&entries),
			..FilesState::default()
		},
	);
	state
}

/// The median and the slowest of 30 frames, after 5 to warm the fonts and layout caches.
fn time(view: View, n: usize) -> (Duration, Duration) {
	let ctx = egui::Context::default();
	let mut state = state_with(view, n);
	for _ in 0..5 {
		frame(&ctx, &mut state, Vec::new());
	}
	let mut times: Vec<Duration> = (0..30)
		.map(|_| {
			let start = Instant::now();
			frame(&ctx, &mut state, Vec::new());
			start.elapsed()
		})
		.collect();
	times.sort();
	(times[15], times[29])
}

#[test]
#[ignore = "a measurement, run in release on demand"]
fn list_frame_times() {
	for view in [View::Changes, View::History, View::Files] {
		for n in [30, 1_000, 5_000] {
			let (median, max) = time(view, n);
			println!(
				"MEASURE {view:?} rows={n} median_ms={:.2} max_ms={:.2}",
				median.as_secs_f64() * 1000.0,
				max.as_secs_f64() * 1000.0
			);
		}
	}
}
