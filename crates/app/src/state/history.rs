//! The History pane's state: pages of commits per repository, the outgoing set, the expanded
//! commit and its files, loaded on demand on background threads and re-read when the
//! repository's status changes.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::log::{LogScope, PAGE, read_commit_files, read_log, read_outgoing};
use repose_core::model::{Commit, CommitFile};

use super::{AppState, Msg, find};
use crate::lanes::{GraphRow, layout};

#[derive(Default)]
pub struct HistoryState {
	/// Bumped by every re-read from page 0; a page carrying an older value is dropped, so a late
	/// result from before a refresh can neither replace the fresh page nor append to it.
	pub generation: u64,
	pub commits: Vec<Commit>,
	/// Hashes on HEAD and not on the upstream; empty without an upstream.
	pub outgoing: HashSet<String>,
	pub loading: bool,
	pub exhausted: bool,
	pub error: Option<String>,
	/// Kept across successful file and later-page reads until the first page can retry it.
	pub outgoing_error: Option<String>,
	pub expanded: Option<String>,
	/// One commit's own file read, kept beside it: a failure shows in that commit's block and
	/// never in `error`, so a late file read can neither clear nor replace a page error.
	pub files: HashMap<String, Result<Vec<CommitFile>, String>>,
	/// One lane row per commit while the lanes view is on and its pages have landed; empty
	/// otherwise, and then the rows draw without a graph.
	pub graph: Vec<GraphRow>,
}

pub struct PageResult {
	pub commits: Vec<Commit>,
	/// Read with the first page only; None means there is no counted upstream.
	pub outgoing: Option<Result<HashSet<String>, String>>,
}

pub enum HistoryMsg {
	Page {
		repo: PathBuf,
		generation: u64,
		skip: usize,
		result: Result<PageResult, String>,
	},
	Files {
		repo: PathBuf,
		hash: String,
		result: Result<Vec<CommitFile>, String>,
	},
}

impl AppState {
	pub fn history_of(&self, repo: &Path) -> Option<&HistoryState> {
		self.history.get(repo)
	}

	/// Loads the first page when the pane first shows a repository.
	pub fn ensure_history(&mut self, repo: &Path, ctx: &egui::Context) {
		if self.history.contains_key(repo) {
			return;
		}
		self.history
			.insert(repo.to_path_buf(), HistoryState::default());
		self.load_history_page(repo, 0, ctx);
	}

	pub fn load_more_history(&mut self, repo: &Path, ctx: &egui::Context) {
		let skip = self.history.get(repo).map(|h| h.commits.len()).unwrap_or(0);
		self.load_history_page(repo, skip, ctx);
	}

	/// Re-reads the first page and the outgoing set in place when a loaded repository's HEAD or
	/// ahead count changed, whether from this app's commit, a Refresh, or a terminal.
	pub(super) fn refresh_history(&mut self, repo: &Path, ctx: &egui::Context) {
		let Some(h) = self.history.get_mut(repo) else {
			return;
		};
		h.generation += 1;
		h.exhausted = false;
		h.loading = false;
		self.load_history_page(repo, 0, ctx);
	}

	/// Turns the lanes view on or off for the session. Each loaded history re-reads its first
	/// page in the other scope, keeping the open commit; the rows on screen stand until it lands.
	pub fn set_lanes(&mut self, on: bool, ctx: &egui::Context) {
		if self.lanes == on {
			return;
		}
		self.lanes = on;
		let loaded: Vec<PathBuf> = self.history.keys().cloned().collect();
		for repo in loaded {
			if let Some(h) = self.history.get_mut(&repo) {
				h.graph.clear();
			}
			self.refresh_history(&repo, ctx);
		}
	}

	/// The upstream to diff against, only when status could count against it: an upstream whose
	/// ref is gone has a name but no ahead/behind, and `<upstream>..HEAD` would fail the page.
	fn upstream_of(&self, repo: &Path) -> Option<String> {
		find(&self.repos, repo)
			.and_then(|r| r.status.as_ref())
			.filter(|s| s.ahead.is_some())
			.and_then(|s| s.upstream.clone())
	}

	fn load_history_page(&mut self, repo: &Path, skip: usize, ctx: &egui::Context) {
		let Some(h) = self.history.get_mut(repo) else {
			return;
		};
		if h.loading || h.exhausted {
			return;
		}
		h.loading = true;
		let generation = h.generation;
		let upstream = if skip == 0 {
			self.upstream_of(repo)
		} else {
			None
		};
		spawn_page(
			PageRequest {
				repo: repo.to_path_buf(),
				generation,
				skip,
				upstream,
				scope: if self.lanes {
					LogScope::All
				} else {
					LogScope::Head
				},
			},
			self.tx.clone(),
			ctx.clone(),
		);
	}

	/// Expands one commit (collapsing any other) and fetches its files once. The Files tab
	/// follows the commit opened here, so its tree can be browsed as it was then.
	pub fn toggle_commit(&mut self, repo: &Path, hash: &str, ctx: &egui::Context) {
		let Some(h) = self.history.get_mut(repo) else {
			return;
		};
		if h.expanded.as_deref() == Some(hash) {
			h.expanded = None;
			return;
		}
		h.expanded = Some(hash.to_string());
		// A failed read is retried when the commit is opened again.
		let fetch = !matches!(h.files.get(hash), Some(Ok(_)));
		self.set_files_rev(repo, super::FileRev::Commit(hash.to_string()));
		if fetch {
			spawn_files(
				repo.to_path_buf(),
				hash.to_string(),
				self.tx.clone(),
				ctx.clone(),
			);
		}
	}

	pub(super) fn merge_history(&mut self, msg: HistoryMsg) {
		match msg {
			HistoryMsg::Page {
				repo,
				generation,
				skip,
				result,
			} => {
				let lanes = self.lanes;
				let Some(h) = self.history.get_mut(&repo) else {
					return;
				};
				if generation != h.generation {
					return;
				}
				h.loading = false;
				match result {
					Ok(page) => {
						h.error = None;
						if page.commits.len() < PAGE {
							h.exhausted = true;
						}
						if skip == 0 {
							h.commits = page.commits;
							match page.outgoing {
								Some(Ok(outgoing)) => {
									h.outgoing = outgoing;
									h.outgoing_error = None;
								}
								Some(Err(error)) => {
									h.error = Some(error.clone());
									h.outgoing_error = Some(error);
								}
								None => {
									h.outgoing.clear();
									h.outgoing_error = None;
								}
							}
						} else {
							h.commits.extend(page.commits);
						}
						// The whole list is laid out again: a lane that a later page continues must
						// keep its column, and the layout is one pass over the commits.
						h.graph = if lanes {
							layout(&h.commits)
						} else {
							Vec::new()
						};
					}
					Err(e) => h.error = Some(e),
				}
			}
			HistoryMsg::Files { repo, hash, result } => {
				// A commit's files are fixed by its hash, so a read from before a page refresh is
				// still right; only the page error must be left alone (audit-logic finding).
				if let Some(h) = self.history.get_mut(&repo) {
					h.files.insert(hash, result);
				}
			}
		}
	}
}

struct PageRequest {
	repo: PathBuf,
	generation: u64,
	skip: usize,
	upstream: Option<String>,
	scope: LogScope,
}

fn spawn_page(req: PageRequest, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let PageRequest {
			repo,
			generation,
			skip,
			upstream,
			scope,
		} = req;
		let result = read_log(&repo, skip, scope)
			.map(|commits| {
				// The outgoing set is extra; a failure to read it leaves the page standing.
				let outgoing = match (skip, &upstream) {
					(0, Some(u)) => Some(read_outgoing(&repo, u).map_err(|e| e.to_string())),
					_ => None,
				};
				PageResult { commits, outgoing }
			})
			.map_err(|e| e.to_string());
		let _ = tx.send(Msg::History(HistoryMsg::Page {
			repo,
			generation,
			skip,
			result,
		}));
		ctx.request_repaint();
	});
}

fn spawn_files(repo: PathBuf, hash: String, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let result = read_commit_files(&repo, &hash).map_err(|e| e.to_string());
		let _ = tx.send(Msg::History(HistoryMsg::Files { repo, hash, result }));
		ctx.request_repaint();
	});
}
