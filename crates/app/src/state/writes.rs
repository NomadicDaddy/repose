//! Stage, unstage, discard, commit and fetch requests. Each repository has at most one write running, so
//! writes never overlap in a repository and a slow hook in one never holds up another; a request
//! behind a running one in the same repository shows as Waiting.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::{Cancel, GitError};
use repose_core::git::{remote, write};

use super::{AppState, Msg};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteKind {
	Stage(Vec<String>),
	StageAll,
	Unstage(Vec<String>),
	UnstageAll,
	Discard {
		tracked: Vec<String>,
		untracked: Vec<String>,
	},
	Commit(String),
	/// `git fetch` from the configured remote, on the user's Fetch (A1, Phillip #35530). It runs
	/// in the same per-repository queue, so it never overlaps a stage or a commit there.
	Fetch,
}

impl WriteKind {
	pub fn label(&self) -> &'static str {
		match self {
			Self::Stage(_) | Self::StageAll => "Staging",
			Self::Unstage(_) | Self::UnstageAll => "Unstaging",
			Self::Discard { .. } => "Discarding",
			Self::Commit(_) => "Committing",
			Self::Fetch => "Fetching",
		}
	}
}

#[derive(Debug, Clone)]
pub struct WriteRequest {
	pub repo: PathBuf,
	pub kind: WriteKind,
	/// Set by the user's Cancel while the request runs; a commit's hooks are stopped with it.
	pub cancel: Cancel,
}

#[derive(Debug, Clone)]
pub struct WriteOutcome {
	pub repo: PathBuf,
	pub kind: WriteKind,
	pub error: Option<String>,
	/// Set when a lock file remained after a failure, with its path; never removed by repose.
	pub leftover_lock: Option<PathBuf>,
	/// A success with a caveat: the commit landed, then Cancel stopped a later hook.
	pub notice: Option<String>,
	/// The error is the user's Cancel, not a failure (a flag, not a match on the text).
	pub cancelled: bool,
	/// The user cancelled and git would not say whether the commit landed; the box says so and
	/// points at History instead of claiming either outcome.
	pub unknown: bool,
}

/// The queue the UI sees: one running request per repository and the ones waiting behind them.
#[derive(Default)]
pub struct WriteQueue {
	pub running: HashMap<PathBuf, WriteRequest>,
	pub waiting: VecDeque<WriteRequest>,
	/// The last outcome per repository, so one repository's failure is not hidden by another's
	/// success now that writes run in parallel across repositories.
	pub last: HashMap<PathBuf, WriteOutcome>,
}

impl WriteQueue {
	/// The user's Cancel on the request running in this repository; the outcome arrives as any
	/// other, with the cancel named as its error.
	pub fn cancel(&self, repo: &Path) {
		if let Some(r) = self.running.get(repo) {
			r.cancel.cancel();
		}
	}

	/// A fetch for this repository is running or waiting.
	pub fn fetching(&self, repo: &Path) -> bool {
		self.running
			.get(repo)
			.is_some_and(|r| r.kind == WriteKind::Fetch)
			|| self
				.waiting
				.iter()
				.any(|r| r.repo == repo && r.kind == WriteKind::Fetch)
	}

	pub fn busy_for(&self, repo: &Path) -> Option<&'static str> {
		if let Some(r) = self.running.get(repo) {
			return Some(r.kind.label());
		}
		if self.waiting.iter().any(|r| r.repo == repo) {
			return Some("Waiting");
		}
		None
	}

	/// The first waiting request whose repository has nothing running, removed from the queue.
	pub fn take_startable(&mut self) -> Option<WriteRequest> {
		let i = self
			.waiting
			.iter()
			.position(|r| !self.running.contains_key(&r.repo))?;
		self.waiting.remove(i)
	}
}

pub fn perform(request: &WriteRequest) -> WriteOutcome {
	let repo = request.repo.as_path();
	let mut notice = None;
	let result = match &request.kind {
		WriteKind::Stage(paths) => write::stage(repo, paths),
		WriteKind::StageAll => write::stage_all(repo),
		WriteKind::Unstage(paths) => write::unstage(repo, paths),
		WriteKind::UnstageAll => write::unstage_all(repo),
		WriteKind::Discard { tracked, untracked } => write::discard(repo, tracked, untracked),
		WriteKind::Fetch => remote::fetch(repo, Some(&request.cancel)),
		WriteKind::Commit(message) => {
			match write::commit_cancellable(repo, message, Some(&request.cancel)) {
				Ok(done) => {
					if done.hook_stopped {
						notice = Some(if done.tree_killed {
							"The commit landed before Cancel; a hook that ran after it was stopped."
								.to_string()
						} else {
							"The commit landed before Cancel; git was stopped, but stopping the later hook's processes failed, so they may still be running."
								.to_string()
						});
					}
					Ok(())
				}
				Err(e) => Err(e),
			}
		}
	};
	let cancelled = matches!(result, Err(GitError::Cancelled { .. }));
	let unknown = matches!(result, Err(GitError::OutcomeUnknown { .. }));
	let error = result.err().map(|e| e.to_string());
	let leftover_lock = if error.is_some() {
		write::index_lock_path(repo).filter(|p| p.exists())
	} else {
		None
	};
	WriteOutcome {
		repo: request.repo.clone(),
		kind: request.kind.clone(),
		error,
		leftover_lock,
		notice,
		cancelled,
		unknown,
	}
}

/// Runs one request on a thread and reports back; the state starts the next when this one lands.
pub fn spawn(request: WriteRequest, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let outcome = perform(&request);
		let _ = tx.send(Msg::WriteDone(Box::new(outcome)));
		ctx.request_repaint();
	});
}

impl AppState {
	/// Queues a write; it runs when its repository has nothing running, in order.
	pub fn request_write(&mut self, repo: PathBuf, kind: WriteKind, ctx: &egui::Context) {
		self.writes.waiting.push_back(WriteRequest {
			repo,
			kind,
			cancel: Cancel::new(),
		});
		self.start_next_write(ctx);
	}

	/// Starts every waiting request whose repository is idle; the rest keep waiting.
	fn start_next_write(&mut self, ctx: &egui::Context) {
		while let Some(next) = self.writes.take_startable() {
			self.writes.running.insert(next.repo.clone(), next.clone());
			spawn(next, self.tx.clone(), ctx.clone());
		}
	}

	/// A successful commit clears its repository's draft; a cancelled or failed one keeps it. A
	/// successful fetch records its time for the "Fetched HH:MM" line; the status re-read below
	/// brings the new ahead and behind counts.
	pub(super) fn finish_write(&mut self, outcome: WriteOutcome, ctx: &egui::Context) {
		if outcome.error.is_none() && matches!(outcome.kind, WriteKind::Commit(_)) {
			self.commit_messages.remove(&outcome.repo);
		}
		if outcome.error.is_none() && outcome.kind == WriteKind::Fetch {
			self.fetched
				.insert(outcome.repo.clone(), std::time::SystemTime::now());
		}
		self.refresh_repo(&outcome.repo.clone(), ctx);
		self.writes.running.remove(&outcome.repo);
		self.writes.last.insert(outcome.repo.clone(), outcome);
		self.start_next_write(ctx);
	}
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
	use std::process::Command;

	use super::*;

	fn git(dir: &Path, args: &[&str]) {
		let status = Command::new("git")
			.current_dir(dir)
			.env("GIT_CONFIG_GLOBAL", "NUL")
			.env("GIT_CONFIG_SYSTEM", "NUL")
			.args(args)
			.status()
			.expect("git runs");
		assert!(status.success(), "git {args:?}");
	}

	/// Morticia L-2 on a31e63b: the leftover-lock report, exercised with a lock that is there
	/// for certain. A planted index.lock makes the commit fail, and the outcome names it; the
	/// test removes what it planted.
	#[test]
	fn a_lock_left_in_the_way_is_reported_by_path_and_not_removed() {
		let dir = std::env::temp_dir().join(format!("repose-lock-test-{}", std::process::id()));
		let _ = std::fs::remove_dir_all(&dir);
		std::fs::create_dir_all(&dir).expect("dir");
		git(&dir, &["init", "-q", "-b", "main"]);
		std::fs::write(dir.join("a.txt"), "a\n").expect("write");
		git(&dir, &["add", "a.txt"]);
		let lock = dir.join(".git").join("index.lock");
		std::fs::write(&lock, "").expect("plant the lock");
		let outcome = perform(&WriteRequest {
			repo: dir.clone(),
			kind: WriteKind::Commit("blocked by a lock".into()),
			cancel: Cancel::new(),
		});
		let still_there = lock.exists();
		let _ = std::fs::remove_file(&lock);
		let _ = std::fs::remove_dir_all(&dir);
		assert!(outcome.error.is_some(), "the commit fails on the lock");
		assert!(!outcome.cancelled);
		assert_eq!(
			outcome.leftover_lock.as_deref(),
			Some(lock.as_path()),
			"the lock is reported by its path: {outcome:?}"
		);
		assert!(still_there, "repose must not remove the lock");
	}
}
