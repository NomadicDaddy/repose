//! A History commit row (screens/history.rs commit_row), drawn through the real frame: a subject
//! wider than its room is painted as one line cut with an ellipsis, never wrapped and clipped
//! (F4 sweep on 6445c56), while a short subject is painted whole.

use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;
use repose_core::model::Commit;

use super::ui_harness::{frame, repo, texts};
use super::{AppState, HistoryState, View};

fn commit(n: u8, subject: &str) -> Commit {
	let hash = format!("{n:02}").repeat(20);
	Commit {
		short: hash[..7].into(),
		hash,
		author: "Ada".into(),
		time: 0,
		date: "1970-01-01T00:00:00Z".into(),
		subject: subject.into(),
		body: String::new(),
		..Commit::default()
	}
}

#[test]
fn a_long_subject_is_one_line_cut_with_an_ellipsis_and_a_short_one_is_whole() {
	let long = format!(
		"Start of a very long subject {}",
		"that keeps going ".repeat(20)
	);
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let row = repo("history-row", SystemTime::now());
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	state.view = View::History;
	let history = HistoryState {
		commits: vec![commit(1, &long), commit(2, "Short subject")],
		exhausted: true,
		..HistoryState::default()
	};
	state.history.insert(path, history);
	let shown = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(shown.iter().any(|t| t == "Short subject"), "{shown:?}");
	assert!(
		!shown
			.iter()
			.any(|t| t == &long || t.contains('\n') && t.starts_with("Start of a very")),
		"the long subject is painted whole or wrapped: {shown:?}"
	);
	let Some(cut) = shown.iter().find(|t| t.starts_with("Start of a very long")) else {
		panic!("no subject painted: {shown:?}");
	};
	assert!(cut.ends_with('…'), "{cut:?}");
	assert!(cut.chars().count() < long.chars().count(), "{cut:?}");
}
