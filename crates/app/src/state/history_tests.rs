//! History state merges: a failed outgoing read keeps the prior set and shows its error, and a
//! commit's file read stays with that commit, never clearing or replacing the page error.

use std::path::PathBuf;

use repose_core::config::Config;
use repose_core::model::Commit;

use super::AppState;
use super::history::{HistoryMsg, HistoryState, PageResult};

#[test]
fn failed_outgoing_read_keeps_prior_hashes_and_shows_error() {
	let mut state = AppState::new(Config::default());
	let repo = PathBuf::from("D:/t/a");
	let mut history = HistoryState::default();
	history.outgoing.insert("prior".into());
	state.history.insert(repo.clone(), history);
	state.merge_history(HistoryMsg::Page {
		repo: repo.clone(),
		generation: 0,
		skip: 0,
		result: Ok(PageResult {
			commits: vec![Commit {
				hash: "new".into(),
				short: "new".into(),
				author: "test".into(),
				time: 0,
				date: "1970-01-01T00:00:00Z".into(),
				subject: "new page".into(),
				body: String::new(),
			}],
			outgoing: Some(Err("outgoing read failed".into())),
		}),
	});
	let Some(history) = state.history.get(&repo) else {
		panic!("history remains loaded");
	};
	assert_eq!(history.commits[0].hash, "new");
	assert!(history.outgoing.contains("prior"));
	assert_eq!(history.error.as_deref(), Some("outgoing read failed"));
	assert_eq!(
		history.outgoing_error.as_deref(),
		Some("outgoing read failed")
	);
	state.merge_history(HistoryMsg::Files {
		repo: repo.clone(),
		hash: "new".into(),
		result: Ok(Vec::new()),
	});
	let Some(history) = state.history.get(&repo) else {
		panic!("history remains loaded after file read");
	};
	assert_eq!(history.error.as_deref(), Some("outgoing read failed"));
	assert_eq!(
		history.outgoing_error.as_deref(),
		Some("outgoing read failed")
	);
}

#[test]
fn a_late_file_read_never_clears_or_replaces_the_page_error() {
	let mut state = AppState::new(Config::default());
	let repo = PathBuf::from("D:/t/b");
	state.history.insert(repo.clone(), HistoryState::default());
	state.merge_history(HistoryMsg::Page {
		repo: repo.clone(),
		generation: 0,
		skip: 0,
		result: Err("log failed".into()),
	});
	for (hash, result) in [
		("ok", Ok(Vec::new())),
		("bad", Err("show failed".to_string())),
	] {
		state.merge_history(HistoryMsg::Files {
			repo: repo.clone(),
			hash: hash.into(),
			result,
		});
	}
	let Some(history) = state.history.get(&repo) else {
		panic!("history remains loaded");
	};
	assert_eq!(history.error.as_deref(), Some("log failed"));
	assert!(matches!(history.files.get("ok"), Some(Ok(f)) if f.is_empty()));
	assert!(matches!(history.files.get("bad"), Some(Err(e)) if e == "show failed"));
}
