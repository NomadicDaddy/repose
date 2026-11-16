//! One repose per config folder (close-to-tray clause 5; Phillip #54737 on #54731). The first
//! process holds an exclusive lock on `repose.lock` beside its config file for as long as it runs.
//! A later launch with the same config cannot take it, so it writes its --select and --view to
//! `repose.handoff` there and exits; the running process picks the file up within a quarter of a
//! second, shows its window, brings it forward and applies them. A run with another `--config`
//! (a test fixture) is its own instance. Safe std only: `File::try_lock`, no Win32 calls.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use eframe::egui;
use repose_core::model::path_key;

use crate::state::{AppState, View};

const LOCK: &str = "repose.lock";
const HANDOFF: &str = "repose.handoff";
const POLL: Duration = Duration::from_millis(250);

/// What a second launch asked the running process for.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Handoff {
	pub select: Option<PathBuf>,
	pub view: Option<View>,
}

/// This process runs the window, holding the lock while it can; or another process holds it.
pub enum Instance {
	/// The lock is `None` when it could not be opened or taken: this process then runs on its own
	/// with no single-instance rule, never refusing to start.
	Primary {
		_lock: Option<File>,
		dir: PathBuf,
	},
	Secondary {
		dir: PathBuf,
	},
}

/// Takes the lock beside `config`, or reports that another repose holds it.
pub fn acquire(config: &Path) -> Instance {
	let dir = config
		.parent()
		.map_or_else(|| PathBuf::from("."), Path::to_path_buf);
	let _ = fs::create_dir_all(&dir);
	let opened = OpenOptions::new()
		.create(true)
		.truncate(false)
		.write(true)
		.open(dir.join(LOCK));
	let Ok(file) = opened else {
		return Instance::Primary { _lock: None, dir };
	};
	match file.try_lock() {
		Ok(()) => Instance::Primary {
			_lock: Some(file),
			dir,
		},
		Err(TryLockError::WouldBlock) => Instance::Secondary { dir },
		Err(TryLockError::Error(_)) => Instance::Primary { _lock: None, dir },
	}
}

/// The running process's lock and the hand-offs arriving for it.
pub struct Running {
	_instance: Instance,
	rx: Receiver<Handoff>,
}

impl Running {
	/// Applies every hand-off that arrived since the last frame.
	pub fn drain(&self, ctx: &egui::Context, state: &mut AppState) {
		for handoff in self.rx.try_iter().collect::<Vec<_>>() {
			apply(ctx, state, handoff);
		}
	}
}

/// main's step before the window opens. A second launch writes its --select, and a --view it was
/// given (`raw` is the command line), as its hand-off and exits 0 here; the first keeps its
/// instance to listen with. The measurement runs always run alone, so a gate measuring startup
/// while the user's repose is open measures a real start.
pub fn claim(config: &Path, args: &crate::cli::Args, raw: &[String]) -> Option<Instance> {
	let measuring = args.measure_startup.is_some()
		|| args.measure_diff.is_some()
		|| args.measure_lanes.is_some();
	if measuring {
		return None;
	}
	match acquire(config) {
		Instance::Secondary { dir } => {
			let handoff = Handoff {
				select: args.select.clone(),
				view: raw.iter().any(|a| a == "--view").then_some(args.view),
			};
			let _ = hand_off(&dir, &handoff);
			std::process::exit(0);
		}
		primary => Some(primary),
	}
}

/// Starts listening for hand-offs once the window's context exists.
pub fn run(instance: Instance, ctx: &egui::Context) -> Running {
	let dir = match &instance {
		Instance::Primary { dir, .. } | Instance::Secondary { dir } => dir.clone(),
	};
	Running {
		rx: listen(dir, ctx.clone()),
		_instance: instance,
	}
}

/// Writes `handoff` for the running process: to a sibling file first, then renamed into place,
/// so it never reads half a file.
pub fn hand_off(dir: &Path, handoff: &Handoff) -> std::io::Result<()> {
	let staged = dir.join(format!("{HANDOFF}.{}", std::process::id()));
	fs::write(&staged, encode(handoff))?;
	fs::rename(&staged, dir.join(HANDOFF))
}

/// Watches for hand-offs in `dir` on a thread, waking a frame for each one.
pub fn listen(dir: PathBuf, ctx: egui::Context) -> Receiver<Handoff> {
	let (tx, rx) = channel();
	let path = dir.join(HANDOFF);
	let _ = fs::remove_file(&path);
	std::thread::spawn(move || {
		loop {
			std::thread::sleep(POLL);
			let Ok(text) = fs::read_to_string(&path) else {
				continue;
			};
			let _ = fs::remove_file(&path);
			if tx.send(decode(&text)).is_err() {
				return;
			}
			ctx.request_repaint();
		}
	});
	rx
}

/// Shows the window, brings it forward and applies the hand-off: the repository it names when it
/// is listed (a worktree under its expanded parent, as at start-up), and the pane.
pub fn apply(ctx: &egui::Context, state: &mut AppState, handoff: Handoff) {
	crate::tray::show_window(ctx, state);
	if let Some(wanted) = handoff.select {
		let key = path_key(&wanted);
		let hit = state.repos.iter().find_map(|r| {
			if path_key(&r.path) == key {
				return Some((r.path.clone(), None));
			}
			let wt = r.worktrees.iter().find(|w| path_key(&w.path) == key)?;
			Some((wt.path.clone(), Some(r.path.clone())))
		});
		if let Some((path, parent)) = hit {
			if let Some(parent) = parent {
				state.expanded.insert(parent, true);
			}
			state.select(path);
		}
	}
	if let Some(view) = handoff.view {
		state.view = view;
	}
	ctx.request_repaint();
}

pub fn encode(handoff: &Handoff) -> String {
	let mut text = String::new();
	if let Some(select) = &handoff.select {
		text.push_str(&format!("select={}\n", select.display()));
	}
	if let Some(view) = handoff.view {
		text.push_str(&format!("view={}\n", view_name(view)));
	}
	text
}

pub fn decode(text: &str) -> Handoff {
	let mut handoff = Handoff::default();
	for line in text.lines() {
		match line.split_once('=') {
			Some(("select", path)) if !path.is_empty() => {
				handoff.select = Some(PathBuf::from(path))
			}
			Some(("view", name)) => handoff.view = crate::cli::view(Some(name)).ok(),
			_ => {}
		}
	}
	handoff
}

fn view_name(view: View) -> &'static str {
	match view {
		View::Changes => "changes",
		View::History => "history",
		View::Files => "files",
		View::Branches => "branches",
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_handoff_round_trips_and_ignores_what_it_does_not_know() {
		let h = Handoff {
			select: Some(PathBuf::from("D:\\applications\\repose")),
			view: Some(View::History),
		};
		assert_eq!(decode(&encode(&h)), h);
		assert_eq!(
			decode("view=sideways\nnoise\nselect=\n"),
			Handoff::default()
		);
		assert_eq!(decode(""), Handoff::default());
	}

	#[test]
	fn a_second_process_with_the_same_config_is_secondary_and_another_config_is_not() {
		let root = std::env::temp_dir().join(format!("repose-single-{}", std::process::id()));
		let _ = fs::remove_dir_all(&root);
		let first = acquire(&root.join("a").join("config.toml"));
		assert!(matches!(first, Instance::Primary { .. }));
		let again = acquire(&root.join("a").join("config.toml"));
		assert!(
			matches!(again, Instance::Secondary { .. }),
			"the lock is held"
		);
		let other = acquire(&root.join("b").join("config.toml"));
		assert!(
			matches!(other, Instance::Primary { .. }),
			"another config folder"
		);
		drop((first, again, other));
		let after = acquire(&root.join("a").join("config.toml"));
		assert!(
			matches!(after, Instance::Primary { .. }),
			"released when the holder ends"
		);
		drop(after);
		let _ = fs::remove_dir_all(&root);
	}
}
