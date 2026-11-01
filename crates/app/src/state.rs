//! The one place UI state lives. Discovery and status run on background threads and arrive as
//! messages tagged with a generation; results are merged by repository path so a refresh never
//! resets the selection or scroll.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;
use repose_core::discovery::discover;
use repose_core::git::read_status_into;
use repose_core::model::RepoSummary;

pub enum Msg {
	Discovered(u64, Vec<RepoSummary>),
	Status(u64, Box<RepoSummary>),
	Done(u64),
}

pub struct AppState {
	pub config: Config,
	pub repos: Vec<RepoSummary>,
	pub selected: Option<PathBuf>,
	pub expanded: HashSet<PathBuf>,
	pub filter: String,
	pub focus_filter: bool,
	pub scanning: bool,
	pub checked_at: Option<SystemTime>,
	/// Configured roots that were not directories when the app started.
	pub missing_roots: Vec<PathBuf>,
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
			expanded: HashSet::new(),
			filter: String::new(),
			focus_filter: false,
			scanning: false,
			checked_at: None,
			generation: 0,
			tx,
			rx,
		}
	}

	/// Walks the roots on a thread, then reads every status through a pool of eight workers.
	pub fn start_scan(&mut self, ctx: &egui::Context) {
		self.generation += 1;
		self.scanning = true;
		let generation = self.generation;
		let config = self.config.clone();
		let tx = self.tx.clone();
		let ctx = ctx.clone();
		std::thread::spawn(move || {
			let repos = discover(&config);
			let mut queue: VecDeque<RepoSummary> = VecDeque::new();
			for r in &repos {
				queue.push_back(r.clone());
				for w in &r.worktrees {
					queue.push_back(w.clone());
				}
			}
			let _ = tx.send(Msg::Discovered(generation, repos));
			ctx.request_repaint();
			let queue = Arc::new(Mutex::new(queue));
			let workers: Vec<_> = (0..8)
				.map(|_| {
					let queue = Arc::clone(&queue);
					let tx = tx.clone();
					let ctx = ctx.clone();
					std::thread::spawn(move || {
						loop {
							let next = queue.lock().ok().and_then(|mut q| q.pop_front());
							let Some(mut repo) = next else { break };
							repo.worktrees.clear();
							read_status_into(&mut repo);
							let _ = tx.send(Msg::Status(generation, Box::new(repo)));
							ctx.request_repaint();
						}
					})
				})
				.collect();
			for w in workers {
				let _ = w.join();
			}
			let _ = tx.send(Msg::Done(generation));
			ctx.request_repaint();
		});
	}

	/// Drains arrived messages into the model. Stale generations are dropped.
	pub fn pump(&mut self, _ctx: &egui::Context) {
		while let Ok(msg) = self.rx.try_recv() {
			match msg {
				Msg::Discovered(g, repos) if g == self.generation => self.merge_discovered(repos),
				Msg::Status(g, repo) if g == self.generation => self.merge_status(*repo),
				Msg::Done(g) if g == self.generation => {
					self.scanning = false;
					self.checked_at = Some(SystemTime::now());
				}
				_ => {}
			}
		}
	}

	fn merge_discovered(&mut self, mut fresh: Vec<RepoSummary>) {
		// Keep statuses already known for paths that are still present.
		for row in &mut fresh {
			if let Some(old) = find(&self.repos, &row.path) {
				row.status = old.status.clone();
				row.error = old.error.clone();
				row.checked_at = old.checked_at;
			}
			for wt in &mut row.worktrees {
				if let Some(old) = find(&self.repos, &wt.path) {
					wt.status = old.status.clone();
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
			row.error = fresh.error;
			row.checked_at = fresh.checked_at;
		}
	}

	pub fn selected_repo(&self) -> Option<&RepoSummary> {
		self.selected.as_ref().and_then(|p| find(&self.repos, p))
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
		self.selected = paths.get(next).cloned();
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
