//! The one place UI state lives. Discovery and status run on background threads and arrive as
//! messages tagged with a generation; results are merged by repository path so a refresh never
//! resets the selection or scroll. Writes go through one serial queue.

mod backstop;
mod diff;
mod diff_size;
mod files;
mod history;
mod merge;
mod refresh;
mod remote;
mod scan;
mod select;
#[cfg(test)]
mod ui_harness;
#[cfg(test)]
mod ui_tests;
mod writes;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;
use repose_core::model::RepoSummary;

pub use diff::{DiffContent, DiffMsg, DiffState, FileColours, OpenDiff};
pub use files::{FileContent, FileRev, FileViewState, FilesMsg, FilesState, OpenFile};
pub use history::{HistoryMsg, HistoryState};
pub use merge::find;
pub use remote::Remote;
pub use writes::{WriteKind, WriteOutcome, WriteQueue};

/// How often every status is re-read even when the watcher reported nothing.
pub const BACKSTOP: std::time::Duration = std::time::Duration::from_secs(60);

pub enum Msg {
	Discovered(u64, Vec<RepoSummary>),
	Status(u64, Box<RepoSummary>),
	/// The scan finished; carries the machine's UTC offset read on the scan thread.
	Done(u64, Option<i32>),
	/// A backstop pass over every row finished, with the UTC offset read on its thread.
	PassDone(u64, Option<i32>),
	WriteDone(Box<WriteOutcome>),
	History(HistoryMsg),
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
}

/// A pending discard, shown in the confirmation dialog until the user decides.
#[derive(Debug, Clone)]
pub struct PendingDiscard {
	pub repo: PathBuf,
	pub tracked: Vec<String>,
	pub untracked: Vec<String>,
}

pub struct AppState {
	pub config: Config,
	pub repos: Vec<RepoSummary>,
	pub selected: Option<PathBuf>,
	pub selected_file: Option<String>,
	pub expanded: HashSet<PathBuf>,
	pub filter: String,
	pub focus_filter: bool,
	pub scanning: bool,
	pub checked_at: Option<SystemTime>,
	/// When the last backstop pass or scan finished (or the app started, before the first).
	backstop_at: Option<std::time::Instant>,
	/// A backstop pass is reading statuses right now; the next waits for it.
	backstop_running: bool,
	/// Configured roots that were not directories at the last scan.
	pub missing_roots: Vec<PathBuf>,
	/// Minutes east of UTC as of the last scan; None shows the clock in UTC.
	pub utc_offset: Option<i32>,
	/// One draft per repository, so a message follows its repository and not the selection.
	pub commit_messages: HashMap<PathBuf, String>,
	pub writes: WriteQueue,
	pub pending_discard: Option<PendingDiscard>,
	pub view: View,
	pub history: HashMap<PathBuf, HistoryState>,
	/// The diff open in the right pane, if any.
	pub diff: Option<DiffState>,
	/// Counts every diff read ever started, so a result from a diff closed and replaced by
	/// another can never match the new one's generation.
	pub(crate) diff_serial: u64,
	/// The Files tab per repository: its revision and tree.
	pub files: HashMap<PathBuf, FilesState>,
	/// The file open read-only in the right pane, if any; never together with `diff`.
	pub file_view: Option<FileViewState>,
	pub(crate) file_serial: u64,
	/// A repository asked for on the command line, applied when discovery first lists it.
	pub requested_selection: Option<PathBuf>,
	/// A working-tree diff asked for on the command line (path, inline), opened on the selected
	/// repository once discovery has run.
	pub requested_diff: Option<(String, bool)>,
	/// A file asked for on the command line, opened read-only at HEAD once the repository lands.
	pub requested_file: Option<String>,
	/// The last external action's failure, shown under the repository header until the next
	/// action succeeds.
	pub notice: Option<String>,
	/// Origins read so far: a URL, none, or why the read failed. Cleared by Refresh.
	pub remotes: HashMap<PathBuf, Remote>,
	/// Repositories whose origin is being read right now.
	pub(crate) remote_reads: HashSet<PathBuf>,
	/// The file-system watcher, its channel and its status-line note.
	pub(crate) watching: refresh::Watching,
	generation: u64,
	tx: Sender<Msg>,
	rx: Receiver<Msg>,
}

impl AppState {
	pub fn new(config: Config) -> Self {
		let (tx, rx) = channel();
		let missing_roots = repose_core::config::missing_roots(&config);
		Self {
			config,
			missing_roots,
			repos: Vec::new(),
			selected: None,
			selected_file: None,
			expanded: HashSet::new(),
			filter: String::new(),
			focus_filter: false,
			scanning: false,
			checked_at: None,
			backstop_at: None,
			backstop_running: false,
			utc_offset: None,
			commit_messages: HashMap::new(),
			writes: WriteQueue::default(),
			pending_discard: None,
			view: View::Changes,
			history: HashMap::new(),
			diff: None,
			diff_serial: 0,
			files: HashMap::new(),
			file_view: None,
			file_serial: 0,
			requested_selection: None,
			requested_diff: None,
			requested_file: None,
			notice: None,
			remotes: HashMap::new(),
			remote_reads: HashSet::new(),
			watching: refresh::Watching::default(),
			generation: 0,
			tx,
			rx,
		}
	}

	/// Walks the roots on a thread, then reads every status through a pool of eight workers.
	pub fn start_scan(&mut self, ctx: &egui::Context) {
		self.generation += 1;
		self.scanning = true;
		self.remotes.clear();
		self.remote_reads.clear();
		self.missing_roots = repose_core::config::missing_roots(&self.config);
		scan::start(
			self.generation,
			self.config.clone(),
			self.tx.clone(),
			ctx.clone(),
		);
	}

	/// Re-reads one repository's status after a write, without rescanning the roots.
	pub fn refresh_repo(&self, path: &Path, ctx: &egui::Context) {
		if let Some(repo) = find(&self.repos, path) {
			scan::refresh_one(self.generation, repo.clone(), self.tx.clone(), ctx.clone());
		}
	}

	/// Drains arrived messages into the model. Stale generations are dropped.
	pub fn pump(&mut self, ctx: &egui::Context) {
		while let Ok(msg) = self.rx.try_recv() {
			match msg {
				Msg::Discovered(g, repos) if g == self.generation => {
					let landed = self.merge_discovered(repos);
					self.sync_watch(ctx);
					// A --file diff opens only on the repository that was asked for; a --select
					// that matched nothing falls back to the first row, and that row's working
					// tree is not the one the caller meant.
					if let Some((path, inline)) = self.requested_diff.take()
						&& landed && let Some(repo) = self.selected.clone()
					{
						self.open_diff_in(
							OpenDiff {
								repo,
								source: repose_core::model::DiffSource::Working,
								paths: vec![path.clone()],
								display: path,
							},
							inline,
							ctx,
						);
					}
					if let Some(path) = self.requested_file.take()
						&& landed && let Some(repo) = self.selected.clone()
					{
						self.open_file(
							OpenFile {
								repo,
								rev: FileRev::Head,
								path,
								size: None,
							},
							ctx,
						);
					}
				}
				Msg::Status(g, repo) if g == self.generation => {
					let path = repo.path.clone();
					let merged = self.merge_status(*repo);
					if merged.head_changed {
						self.refresh_history(&path, ctx);
						self.refresh_files_for(&path, ctx);
					}
					if merged.applied {
						self.reload_diff_after_status(&path, merged.changed, ctx);
					}
				}
				Msg::Done(g, offset) if g == self.generation => {
					self.scanning = false;
					self.pass_done(offset, ctx);
				}
				Msg::PassDone(g, offset) if g == self.generation => self.pass_done(offset, ctx),
				Msg::Notice(text) => self.notice = Some(text),
				Msg::WriteDone(outcome) => self.finish_write(*outcome, ctx),
				Msg::History(h) => self.merge_history(h),
				Msg::Diff(m) => self.merge_diff(m),
				Msg::Files(m) => self.merge_files(m),
				Msg::Remote(g, repo, remote) => self.merge_remote(g, repo, remote),
				_ => {}
			}
		}
	}

	/// For work done on a thread that has something to report back (a failed external action).
	pub(crate) fn sender(&self) -> Sender<Msg> {
		self.tx.clone()
	}

	pub fn selected_repo(&self) -> Option<&RepoSummary> {
		self.selected.as_ref().and_then(|p| find(&self.repos, p))
	}

	/// Selects a repository. An open diff or file belongs to the repository it was opened on, so
	/// moving to another closes it rather than drawing it under the new header.
	pub fn select(&mut self, path: PathBuf) {
		if self.selected.as_ref() != Some(&path) {
			self.selected_file = None;
			if self.diff.as_ref().is_some_and(|d| d.open.repo != path) {
				self.close_diff();
			}
			if self.file_view.as_ref().is_some_and(|v| v.open.repo != path) {
				self.close_file();
			}
		}
		self.selected = Some(path);
	}
}
