//! The one place UI state lives. Discovery and status run on background threads and arrive as
//! messages tagged with a generation; results are merged by repository path so a refresh never
//! resets the selection or scroll. Writes go through one serial queue.

mod backstop;
mod branches;
mod diff;
mod diff_size;
mod files;
mod history;
#[cfg(test)]
mod history_tests;
mod merge;
mod msg;
mod pump;
mod refresh;
mod remote;
mod scan;
mod select;
#[cfg(test)]
mod selection_tests;
#[cfg(test)]
mod ui_a11y_tests;
#[cfg(test)]
mod ui_action_failure_tests;
#[cfg(test)]
mod ui_branch_menu_tests;
#[cfg(test)]
mod ui_close_guard_tests;
#[cfg(test)]
mod ui_commit_outcome_tests;
#[cfg(test)]
mod ui_discard_a11y_tests;
#[cfg(test)]
mod ui_discard_tests;
#[cfg(test)]
mod ui_escape_tests;
#[cfg(test)]
mod ui_fetch_tests;
#[cfg(test)]
mod ui_filter_tests;
#[cfg(test)]
mod ui_focus_tests;
#[cfg(test)]
mod ui_harness;
#[cfg(test)]
mod ui_header_tests;
#[cfg(test)]
mod ui_history_block_tests;
#[cfg(test)]
mod ui_history_row_tests;
#[cfg(test)]
mod ui_labels_tests;
#[cfg(test)]
mod ui_lanes_tests;
#[cfg(test)]
mod ui_live_tests;
#[cfg(test)]
mod ui_origin_link_tests;
#[cfg(test)]
mod ui_repo_list_tests;
#[cfg(test)]
mod ui_row_activation_tests;
#[cfg(test)]
mod ui_row_focus_tests;
#[cfg(test)]
mod ui_tests;
#[cfg(test)]
mod ui_theme_tests;
mod writes;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;
use repose_core::model::RepoSummary;

pub use branches::{BranchesMsg, BranchesState};
pub use diff::{DiffContent, DiffMsg, DiffState, FileColours, OpenDiff};
pub use files::{FileContent, FileRev, FileViewState, FilesMsg, FilesState, OpenFile};
pub use history::{HistoryMsg, HistoryState};
pub use merge::find;
pub use msg::{Msg, PendingDiscard, View};
pub use remote::Remote;
pub use writes::{WriteKind, WriteOutcome, WriteQueue};

/// How often every status is re-read even when the watcher reported nothing.
pub const BACKSTOP: std::time::Duration = std::time::Duration::from_secs(60);

pub struct AppState {
	pub config: Config,
	pub repos: Vec<RepoSummary>,
	pub selected: Option<PathBuf>,
	pub selected_file: Option<String>,
	/// The selection just moved by something other than a click on its row (a keyboard move, a
	/// start-up or command-line selection, a vanished repository), so the list scrolls the
	/// selected row into view once and then clears this. Never every frame: that would fight
	/// the wheel (Morticia L-2 on 4b39403).
	pub reveal_selected: bool,
	/// A repository the user opened (true) or closed (false) by its chevron. A repository with
	/// no entry is left to the filter: open to the worktrees whose names match while a filter is
	/// typed, closed otherwise. An entry overrides the filter either way, so a click always
	/// changes the rows (Morticia L-6 on 4dc97d1).
	pub expanded: HashMap<PathBuf, bool>,
	pub filter: String,
	pub focus_filter: bool,
	/// Letters typed with the list focused (Phillip #31623): the prefix so far and when its last
	/// letter arrived. It selects the first visible row whose name starts with it and starts over
	/// after a second's pause.
	pub type_ahead: String,
	pub type_ahead_at: Option<std::time::Instant>,
	/// A text box (the Find box, a commit message) had focus on the last frame, so typed letters
	/// are its own and never move the selection. Each frame clears it and the boxes set it.
	pub text_focus: bool,
	/// A row of the Branches tab had focus on the last frame, so Up and Down move within that
	/// list and leave the repository selection where it is (branches-list clause 5).
	pub rows_focus: bool,
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
	/// When each repository's last Fetch succeeded, for its "Fetched HH:MM" line. Kept across
	/// Refresh: the refs it brought stay fetched.
	pub fetched: HashMap<PathBuf, std::time::SystemTime>,
	/// The repositories whose unsaved draft held up a window close, while the question is open.
	pub close_prompt: Option<Vec<String>>,
	/// The user chose to close without committing; the next close request goes through.
	pub close_confirmed: bool,
	pub writes: WriteQueue,
	pub pending_discard: Option<PendingDiscard>,
	pub view: View,
	pub history: HashMap<PathBuf, HistoryState>,
	/// History draws branch lanes over every ref (branch-graph-lanes); off at start, kept for the
	/// session.
	pub lanes: bool,
	/// The Branches tab per repository, read when the tab first shows it.
	pub branches: HashMap<PathBuf, BranchesState>,
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
			reveal_selected: false,
			expanded: HashMap::new(),
			filter: String::new(),
			focus_filter: false,
			type_ahead: String::new(),
			type_ahead_at: None,
			text_focus: false,
			rows_focus: false,
			scanning: false,
			checked_at: None,
			backstop_at: None,
			backstop_running: false,
			utc_offset: None,
			commit_messages: HashMap::new(),
			fetched: HashMap::new(),
			close_prompt: None,
			close_confirmed: false,
			writes: WriteQueue::default(),
			pending_discard: None,
			view: View::Changes,
			history: HashMap::new(),
			lanes: false,
			branches: HashMap::new(),
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

	/// Whether the discard confirmation or the close question stands. A Modal blocks pointer
	/// input and focus behind it, not raw key reads, so a key the dialog answers (Escape) must
	/// not also act on the pane behind it.
	pub fn modal_open(&self) -> bool {
		self.pending_discard.is_some() || self.close_prompt.is_some()
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
