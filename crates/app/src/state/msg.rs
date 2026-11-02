//! What the threads send back to the model, which pane the right side shows, and a discard
//! waiting for its confirmation.

use std::path::PathBuf;

use repose_core::model::RepoSummary;

use super::{BranchesMsg, DiffMsg, FilesMsg, HistoryMsg, Remote, WriteOutcome};

pub enum Msg {
	Discovered(u64, Vec<RepoSummary>),
	Status(u64, Box<RepoSummary>),
	/// The scan finished; carries the machine's UTC offset read on the scan thread.
	Done(u64, Option<i32>),
	/// A backstop pass over every row finished, with the UTC offset read on its thread.
	PassDone(u64, Option<i32>),
	WriteDone(Box<WriteOutcome>),
	History(HistoryMsg),
	Branches(BranchesMsg),
	Diff(DiffMsg),
	Files(FilesMsg),
	/// The origin of a repository, read under a scan generation: a URL, none, or the failure.
	Remote(u64, PathBuf, Remote),
	/// An external action that ran on a thread failed; shown under the repository header.
	Notice(String),
}

/// Which pane the right side shows for the selected repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
	Changes,
	History,
	Files,
	Branches,
}

/// A pending discard, shown in the confirmation dialog until the user decides.
#[derive(Debug, Clone)]
pub struct PendingDiscard {
	pub repo: PathBuf,
	pub tracked: Vec<String>,
	pub untracked: Vec<String>,
}
