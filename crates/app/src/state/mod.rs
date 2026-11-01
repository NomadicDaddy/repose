//! The one place UI state lives. Discovery and status run on background threads and arrive as
//! messages tagged with a generation; results are merged by repository path so a refresh never
//! resets the selection or scroll. Writes go through one serial queue.

mod scan;
mod writes;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;
use repose_core::model::RepoSummary;

pub use writes::{WriteKind, WriteOutcome, WriteQueue, WriteRequest};

pub enum Msg {
	Discovered(u64, Vec<RepoSummary>),
	Status(u64, Box<RepoSummary>),
	/// The scan finished; carries the machine's UTC offset read on the scan thread.
	Done(u64, Option<i32>),
	WriteDone(Box<WriteOutcome>),
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
	/// Configured roots that were not directories at the last scan.
	pub missing_roots: Vec<PathBuf>,
	/// Minutes east of UTC as of the last scan; None shows the clock in UTC.
	pub utc_offset: Option<i32>,
	pub commit_message: String,
	pub writes: WriteQueue,
	pub pending_discard: Option<PendingDiscard>,
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
			utc_offset: None,
			commit_message: String::new(),
			writes: WriteQueue::default(),
			pending_discard: None,
			generation: 0,
			tx,
			rx,
		}
	}

	/// Walks the roots on a thread, then reads every status through a pool of eight workers.
	pub fn start_scan(&mut self, ctx: &egui::Context) {
		self.generation += 1;
		self.scanning = true;
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
				Msg::Discovered(g, repos) if g == self.generation => self.merge_discovered(repos),
				Msg::Status(g, repo) if g == self.generation => self.merge_status(*repo),
				Msg::Done(g, offset) if g == self.generation => {
					self.scanning = false;
					self.checked_at = Some(SystemTime::now());
					self.utc_offset = offset;
				}
				Msg::WriteDone(outcome) => self.finish_write(*outcome, ctx),
				_ => {}
			}
		}
	}

	fn merge_discovered(&mut self, mut fresh: Vec<RepoSummary>) {
		// Keep statuses already known for paths that are still present.
		for row in &mut fresh {
			if let Some(old) = find(&self.repos, &row.path) {
				row.status = old.status.clone();
				row.changes = old.changes.clone();
				row.error = old.error.clone();
				row.checked_at = old.checked_at;
			}
			for wt in &mut row.worktrees {
				if let Some(old) = find(&self.repos, &wt.path) {
					wt.status = old.status.clone();
					wt.changes = old.changes.clone();
					wt.error = old.error.clone();
				}
			}
		}
		self.repos = fresh;
		if self
			.selected
			.as_ref()
			.is_none_or(|p| find(&self.repos, p).is_none())
		{
			self.selected = self.repos.first().map(|r| r.path.clone());
		}
	}

	fn merge_status(&mut self, fresh: RepoSummary) {
		if let Some(row) = find_mut(&mut self.repos, &fresh.path) {
			row.branch = fresh.branch;
			row.short_hash = fresh.short_hash.or(row.short_hash.take());
			row.status = fresh.status;
			row.changes = fresh.changes;
			row.error = fresh.error;
			row.checked_at = fresh.checked_at;
		}
	}

	pub fn selected_repo(&self) -> Option<&RepoSummary> {
		self.selected.as_ref().and_then(|p| find(&self.repos, p))
	}

	pub fn select(&mut self, path: PathBuf) {
		if self.selected.as_ref() != Some(&path) {
			self.selected_file = None;
		}
		self.selected = Some(path);
	}

	/// Queues a write; it runs when the writer is free, in order.
	pub fn request_write(&mut self, repo: PathBuf, kind: WriteKind, ctx: &egui::Context) {
		self.writes.waiting.push_back(WriteRequest { repo, kind });
		self.start_next_write(ctx);
	}

	fn start_next_write(&mut self, ctx: &egui::Context) {
		if self.writes.running.is_some() {
			return;
		}
		if let Some(next) = self.writes.waiting.pop_front() {
			self.writes.running = Some(next.clone());
			writes::spawn(next, self.tx.clone(), ctx.clone());
		}
	}

	fn finish_write(&mut self, outcome: WriteOutcome, ctx: &egui::Context) {
		if outcome.error.is_none() && matches!(outcome.kind, WriteKind::Commit(_)) {
			self.commit_message.clear();
		}
		self.refresh_repo(&outcome.repo.clone(), ctx);
		self.writes.last = Some(outcome);
		self.writes.running = None;
		self.start_next_write(ctx);
	}

	/// Rows in display order after the filter: top-level repositories plus the worktrees of the
	/// expanded ones. Used for both drawing and keyboard movement.
	pub fn visible_paths(&self) -> Vec<PathBuf> {
		let needle = self.filter.trim().to_lowercase();
		let mut out = Vec::new();
		for r in &self.repos {
			let matches_self = needle.is_empty() || r.name.to_lowercase().contains(&needle);
			let matching_wts: Vec<&RepoSummary> = r
				.worktrees
				.iter()
				.filter(|w| needle.is_empty() || w.name.to_lowercase().contains(&needle))
				.collect();
			if !matches_self && matching_wts.is_empty() {
				continue;
			}
			out.push(r.path.clone());
			if self.expanded.contains(&r.path) || (!needle.is_empty() && !matching_wts.is_empty()) {
				for w in matching_wts {
					out.push(w.path.clone());
				}
			}
		}
		out
	}

	pub fn move_selection(&mut self, delta: i32) {
		let paths = self.visible_paths();
		if paths.is_empty() {
			return;
		}
		let current = self
			.selected
			.as_ref()
			.and_then(|s| paths.iter().position(|p| p == s));
		let next = match current {
			Some(i) => (i as i32 + delta).clamp(0, paths.len() as i32 - 1) as usize,
			None => 0,
		};
		if let Some(p) = paths.get(next).cloned() {
			self.select(p);
		}
	}
}

pub fn find<'a>(repos: &'a [RepoSummary], path: &Path) -> Option<&'a RepoSummary> {
	for r in repos {
		if r.path == path {
			return Some(r);
		}
		if let Some(w) = r.worktrees.iter().find(|w| w.path == path) {
			return Some(w);
		}
	}
	None
}

fn find_mut<'a>(repos: &'a mut [RepoSummary], path: &Path) -> Option<&'a mut RepoSummary> {
	for r in repos.iter_mut() {
		if r.path == path {
			return Some(r);
		}
		if let Some(w) = r.worktrees.iter_mut().find(|w| w.path == path) {
			return Some(w);
		}
	}
	None
}
