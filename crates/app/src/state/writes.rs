//! Stage, unstage, discard and commit requests. Each repository has at most one write running, so
//! writes never overlap in a repository and a slow hook in one never holds up another; a request
//! behind a running one in the same repository shows as Waiting.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::write;

use super::Msg;

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
}

impl WriteKind {
	pub fn label(&self) -> &'static str {
		match self {
			Self::Stage(_) | Self::StageAll => "Staging",
			Self::Unstage(_) | Self::UnstageAll => "Unstaging",
			Self::Discard { .. } => "Discarding",
			Self::Commit(_) => "Committing",
		}
	}
}

#[derive(Debug, Clone)]
pub struct WriteRequest {
	pub repo: PathBuf,
	pub kind: WriteKind,
}

#[derive(Debug, Clone)]
pub struct WriteOutcome {
	pub repo: PathBuf,
	pub kind: WriteKind,
	pub error: Option<String>,
	/// Set when a lock file remained after a failure, with its path; never removed by repose.
	pub leftover_lock: Option<PathBuf>,
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
	let result = match &request.kind {
		WriteKind::Stage(paths) => write::stage(repo, paths),
		WriteKind::StageAll => write::stage_all(repo),
		WriteKind::Unstage(paths) => write::unstage(repo, paths),
		WriteKind::UnstageAll => write::unstage_all(repo),
		WriteKind::Discard { tracked, untracked } => write::discard(repo, tracked, untracked),
		WriteKind::Commit(message) => write::commit(repo, message),
	};
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
