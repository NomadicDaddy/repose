//! The OS watcher: one recursive notify watch per configured root. Events are mapped to their
//! repository and filtered on the watcher's own thread, handed to the debouncer, and delivered
//! as refreshes through a channel. An overflow (the OS dropped events) or a watcher error is
//! reported as `Refresh::All`, so the app re-reads everything once rather than guessing.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use super::{Debouncer, RepoMap, keep};

/// What the app should re-read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refresh {
	Repo(PathBuf),
	/// Events were lost or the watcher failed; every status is stale.
	All,
}

/// A root that could not be watched, with the OS's reason.
#[derive(Debug, Clone)]
pub struct Unwatched {
	pub root: PathBuf,
	pub reason: String,
}

pub struct Watch {
	_watcher: RecommendedWatcher,
	map: Arc<Mutex<RepoMap>>,
}

impl Watch {
	/// Watches `roots` and sends refreshes on `out`, calling `wake` after each send so a UI can
	/// repaint. A root that cannot be watched is returned, the rest are watched; only a watcher
	/// that cannot be created at all is an error.
	pub fn start(
		roots: &[PathBuf],
		exclude: Vec<String>,
		map: RepoMap,
		out: Sender<Refresh>,
		wake: Box<dyn Fn() + Send>,
	) -> Result<(Watch, Vec<Unwatched>), String> {
		let (ev_tx, ev_rx) = channel::<notify::Result<Event>>();
		let mut watcher = notify::recommended_watcher(ev_tx).map_err(|e| e.to_string())?;
		let mut unwatched = Vec::new();
		for root in roots {
			if let Err(e) = watcher.watch(root, RecursiveMode::Recursive) {
				unwatched.push(Unwatched {
					root: root.clone(),
					reason: e.to_string(),
				});
			}
		}
		let map = Arc::new(Mutex::new(map));
		let shared = Arc::clone(&map);
		std::thread::spawn(move || run(ev_rx, shared, exclude, out, wake));
		Ok((
			Watch {
				_watcher: watcher,
				map,
			},
			unwatched,
		))
	}

	/// Replaces the repository map after a rescan found different rows.
	pub fn set_map(&self, map: RepoMap) {
		if let Ok(mut m) = self.map.lock() {
			*m = map;
		}
	}
}

/// The watcher thread: events in, debounced refreshes out, until either channel closes.
fn run(
	rx: Receiver<notify::Result<Event>>,
	map: Arc<Mutex<RepoMap>>,
	exclude: Vec<String>,
	out: Sender<Refresh>,
	wake: Box<dyn Fn() + Send>,
) {
	let mut debouncer = Debouncer::default();
	loop {
		let received = match debouncer.next_deadline() {
			Some(deadline) => rx.recv_timeout(deadline.saturating_duration_since(Instant::now())),
			None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
		};
		match received {
			Ok(Ok(event)) => {
				if event.need_rescan() {
					if out.send(Refresh::All).is_err() {
						return;
					}
					wake();
				} else if !matches!(event.kind, EventKind::Access(_)) {
					let now = Instant::now();
					if let Ok(map) = map.lock() {
						for path in &event.paths {
							if let Some(m) = map.owner(path)
								&& keep(&m.rel, &exclude)
							{
								debouncer.note(m.repo, now);
							}
						}
					}
				}
			}
			Ok(Err(_)) => {
				if out.send(Refresh::All).is_err() {
					return;
				}
				wake();
			}
			Err(RecvTimeoutError::Timeout) => {}
			Err(RecvTimeoutError::Disconnected) => return,
		}
		let due = debouncer.due(Instant::now());
		if !due.is_empty() {
			for repo in due {
				if out.send(Refresh::Repo(repo)).is_err() {
					return;
				}
			}
			wake();
		}
	}
}
