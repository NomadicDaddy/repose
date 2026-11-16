//! One repose per config folder (close-to-tray clause 5; Phillip #54737 on #54731). The first
//! process holds an exclusive lock on `repose.lock` beside its config file for as long as it runs.
//! A later launch with a config in the same folder cannot take it, so it writes what it was asked
//! for (--select, a --view it was given, --file with --inline, --open and --lanes) to
//! `repose.handoff` there and exits 0, or 2 if that write fails; the running process picks the file
//! up within a quarter of a second, shows its window, brings it forward and applies it as a start-up
//! would. A run with a config in another folder (a test fixture) is its own instance. Safe std
//! only: `File::try_lock`, no Win32 calls.

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

/// What a second launch asked the running process for: every flag a start-up acts on, so a launch
/// does the same whether or not repose is already running (Roger 5Q-5 on 0a3048d).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Handoff {
	pub select: Option<PathBuf>,
	pub view: Option<View>,
	/// `--file` and whether `--inline` came with it.
	pub file: Option<(String, bool)>,
	pub open: Option<String>,
	pub lanes: bool,
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

/// main's step before the window opens. A second launch writes its hand-off (`raw` is the command
/// line, for whether --view was given) and exits here: 0 once it is written, 2 with the reason when
/// it cannot be (Roger 5Q-1 on 0a3048d). The first keeps its instance to listen with. The
/// measurement runs always run alone, so a gate measuring startup while the user's repose is open
/// measures a real start.
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
				file: args.file.clone().map(|f| (f, args.inline)),
				open: args.open.clone(),
				lanes: args.lanes,
			};
			if let Err(e) = hand_off(&dir, &handoff) {
				eprintln!(
					"repose: could not hand off to the running repose in {}: {e}",
					dir.display()
				);
				std::process::exit(2);
			}
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

/// Watches for hand-offs in `dir` on a thread, waking a frame for each one. A hand-off already
/// there when it starts is applied, not deleted: a second launch can write it between this
/// process taking the lock and the window existing (Roger 5Q-1 on 0a3048d).
pub fn listen(dir: PathBuf, ctx: egui::Context) -> Receiver<Handoff> {
	let (tx, rx) = channel();
	let path = dir.join(HANDOFF);
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

/// Shows the window, brings it forward and applies the hand-off as a start-up would. A repository
/// already listed is selected at once (a worktree under its expanded parent); one not listed yet,
/// during the first scan, waits for that scan's result as a start-up --select does (Roger 5Q-1).
/// --file and --open take the start-up route too: they are requested, and a scan lands them on
/// the repository once it is selected (5Q-5). --view and --lanes apply at once.
pub fn apply(ctx: &egui::Context, state: &mut AppState, handoff: Handoff) {
	crate::tray::show_window(ctx, state);
	if let Some(wanted) = &handoff.select {
		let key = path_key(wanted);
		let hit = state.repos.iter().find_map(|r| {
			if path_key(&r.path) == key {
				return Some((r.path.clone(), None));
			}
			let wt = r.worktrees.iter().find(|w| path_key(&w.path) == key)?;
			Some((wt.path.clone(), Some(r.path.clone())))
		});
		match hit {
			Some((path, parent)) => {
				if let Some(parent) = parent {
					state.expanded.insert(parent, true);
				}
				state.select(path);
			}
			None => state.requested_selection = Some(wanted.clone()),
		}
	}
	if let Some(view) = handoff.view {
		state.view = view;
	}
	if handoff.lanes {
		state.lanes = true;
	}
	let wants_open = handoff.file.is_some() || handoff.open.is_some();
	if wants_open {
		state.requested_diff = handoff.file;
		state.requested_file = handoff.open;
		if state.requested_selection.is_none() {
			state.requested_selection = handoff.select.or_else(|| state.selected.clone());
		}
	}
	if (wants_open || state.requested_selection.is_some()) && !state.scanning {
		state.start_scan(ctx);
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
	if let Some((file, inline)) = &handoff.file {
		text.push_str(&format!("file={file}\n"));
		if *inline {
			text.push_str("inline=1\n");
		}
	}
	if let Some(open) = &handoff.open {
		text.push_str(&format!("open={open}\n"));
	}
	if handoff.lanes {
		text.push_str("lanes=1\n");
	}
	text
}

pub fn decode(text: &str) -> Handoff {
	let mut handoff = Handoff::default();
	let mut inline = false;
	for line in text.lines() {
		match line.split_once('=') {
			Some(("select", path)) if !path.is_empty() => {
				handoff.select = Some(PathBuf::from(path))
			}
			Some(("view", name)) => handoff.view = crate::cli::view(Some(name)).ok(),
			Some(("file", path)) if !path.is_empty() => handoff.file = Some((path.into(), false)),
			Some(("inline", "1")) => inline = true,
			Some(("open", path)) if !path.is_empty() => handoff.open = Some(path.into()),
			Some(("lanes", "1")) => handoff.lanes = true,
			_ => {}
		}
	}
	if let Some((_, i)) = &mut handoff.file {
		*i = inline;
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
