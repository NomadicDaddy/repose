//! Plain data that crosses from core to the app.

mod branch;
mod changes;
mod commit;
mod diff;
mod tree;

use std::path::PathBuf;
use std::time::SystemTime;

pub use branch::{Branch, Branches, HeadState};
pub use changes::{ChangeKind, Changes, DiscardPlan, FileChange};
pub use commit::{Commit, CommitFile, parse_iso, relative};
pub use diff::{DiffLine, DiffSource, FileDiff, Hunk, LineKind};
pub use tree::TreeEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoKind {
	Repository,
	Worktree,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchState {
	Branch(String),
	Detached,
	Unborn,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusSummary {
	pub staged: usize,
	pub unstaged: usize,
	pub untracked: usize,
	pub conflicts: usize,
	pub upstream: Option<String>,
	pub ahead: Option<u32>,
	pub behind: Option<u32>,
}

impl StatusSummary {
	pub fn changed(&self) -> usize {
		self.staged + self.unstaged + self.untracked + self.conflicts
	}
}

#[derive(Debug, Clone)]
pub struct RepoSummary {
	pub path: PathBuf,
	pub name: String,
	pub kind: RepoKind,
	/// For a worktree, the main repository it belongs to.
	pub main_path: Option<PathBuf>,
	/// For a worktree, its `<main>/.git/worktrees/<name>` folder, where its HEAD and index live.
	pub gitdir: Option<PathBuf>,
	pub branch: BranchState,
	pub short_hash: Option<String>,
	pub submodules: usize,
	pub status: Option<StatusSummary>,
	/// The grouped file list behind `status`, for the Changes view.
	pub changes: Option<Changes>,
	pub error: Option<String>,
	pub checked_at: Option<SystemTime>,
	pub worktrees: Vec<RepoSummary>,
}

impl RepoSummary {
	pub fn new(path: PathBuf, kind: RepoKind) -> Self {
		let name = path
			.file_name()
			.map(|n| n.to_string_lossy().to_string())
			.unwrap_or_default();
		Self {
			path,
			name,
			kind,
			main_path: None,
			gitdir: None,
			branch: BranchState::Unborn,
			short_hash: None,
			submodules: 0,
			status: None,
			changes: None,
			error: None,
			checked_at: None,
			worktrees: Vec::new(),
		}
	}

	/// The second line of a repository row: `main · ce0cc27`, `detached · ce0cc27`, `no commits`.
	pub fn branch_line(&self) -> String {
		match (&self.branch, &self.short_hash) {
			(BranchState::Branch(b), Some(h)) => format!("{b} · {h}"),
			(BranchState::Branch(b), None) => b.clone(),
			(BranchState::Detached, Some(h)) => format!("detached · {h}"),
			(BranchState::Detached, None) => "detached".into(),
			(BranchState::Unborn, _) => "no commits".into(),
		}
	}
}
