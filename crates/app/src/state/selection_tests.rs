//! A requested selection lands on its row whatever spelling `--select` was given: a trailing
//! separator (PowerShell tab completion adds one), backslashes and different case all name the
//! same repository (audit-architecture path identity).

use std::path::PathBuf;

use repose_core::config::Config;
use repose_core::model::{RepoKind, RepoSummary};

use super::AppState;

#[test]
fn a_selection_with_a_trailing_separator_and_other_case_lands_on_its_row() {
	let mut state = AppState::new(Config::default());
	let other = RepoSummary::new(PathBuf::from("D:/t/alpha"), RepoKind::Repository);
	let wanted = RepoSummary::new(PathBuf::from("D:/t/Repose"), RepoKind::Repository);
	state.requested_selection = Some(PathBuf::from("d:\\t\\repose\\"));
	assert!(
		state.merge_discovered(vec![other, wanted]),
		"the request landed"
	);
	assert_eq!(state.selected, Some(PathBuf::from("D:/t/Repose")));
}
