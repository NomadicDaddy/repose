//! The settings screen's draft (settings-screen clauses 1 to 4): the config's roots, excludes,
//! depth, theme and commands copied when the screen opens, edited there, checked as they are
//! typed, and either saved to config.toml and applied at once (Save) or dropped (Cancel, Escape).

use std::path::{Path, PathBuf};

use eframe::egui;
use repose_core::config::{self, Config, Theme};

use super::AppState;

/// The deepest scan the depth control offers; deeper walks get slow on large drives.
pub const MAX_DEPTH: u32 = 8;

/// A line under an entry: its text, and whether it keeps Save disabled.
#[derive(Clone, Copy)]
pub struct Mark {
	pub text: &'static str,
	pub blocks: bool,
}

pub struct SettingsDraft {
	pub roots: Vec<String>,
	/// The roots as the config held them when the screen opened.
	saved_roots: Vec<String>,
	/// The text in the box under the roots, added by its Add button.
	pub new_root: String,
	pub exclude: Vec<String>,
	pub new_exclude: String,
	pub depth: u32,
	pub theme: Theme,
	pub editor: String,
	pub terminal: String,
	/// Why the last Save failed, shown above the buttons until the next Save.
	pub error: Option<String>,
	/// The screen opened this frame, so its first box takes focus once.
	pub fresh: bool,
}

impl SettingsDraft {
	fn from_config(config: &Config) -> Self {
		let roots: Vec<String> = config
			.roots
			.iter()
			.map(|r| r.display().to_string())
			.collect();
		Self {
			saved_roots: roots.clone(),
			roots,
			new_root: String::new(),
			exclude: config.exclude.clone(),
			new_exclude: String::new(),
			depth: config.depth,
			theme: config.theme,
			editor: config.commands.editor.clone(),
			terminal: config.commands.terminal.clone(),
			error: None,
			fresh: true,
		}
	}

	/// The mark under a root. A folder that exists has none. A root the config already held that
	/// is missing now is only noted: the load tolerates it so one unplugged drive never hides the
	/// others (config::validate), and it must not hold up every other change either (Morticia
	/// 5Q-3 on 25e8094). A newly added root that does not exist blocks Save.
	pub fn root_mark(&self, root: &str) -> Option<Mark> {
		let root = root.trim();
		if Path::new(root).is_dir() {
			return None;
		}
		let saved = self
			.saved_roots
			.iter()
			.any(|s| s.eq_ignore_ascii_case(root));
		Some(if saved {
			Mark {
				text: "Not found now. Kept, since a drive may be unplugged.",
				blocks: false,
			}
		} else {
			Mark {
				text: "This folder does not exist.",
				blocks: true,
			}
		})
	}

	/// What is wrong with a command, for the line under its box.
	pub fn command_problem(command: &str) -> Option<&'static str> {
		(!config::command_ok(command.trim()))
			.then_some("Use a program name or an absolute path, with no spaces or arguments.")
	}

	/// The mark under an excluded name: it is a folder name, never a path.
	pub fn exclude_mark(name: &str) -> Option<Mark> {
		let name = name.trim();
		(name.is_empty() || name.contains(['/', '\\'])).then_some(Mark {
			text: "Use a folder name, without slashes.",
			blocks: true,
		})
	}

	/// Save is offered only when no mark blocks it (settings-screen clause 2) and neither New box
	/// holds text that was never added, which Save would otherwise drop (Morticia 5Q-2).
	pub fn ready(&self) -> bool {
		let blocks = |m: Option<Mark>| m.is_some_and(|m| m.blocks);
		!self.roots.iter().any(|r| blocks(self.root_mark(r)))
			&& !self.exclude.iter().any(|e| blocks(Self::exclude_mark(e)))
			&& Self::command_problem(&self.editor).is_none()
			&& Self::command_problem(&self.terminal).is_none()
			&& self.new_root.trim().is_empty()
			&& self.new_exclude.trim().is_empty()
	}

	/// `base` with this draft's settings in place of its own; the pins and anything else stay.
	/// The depth is taken as it stands: the control never clamps a value the file already held,
	/// so a save that did not touch it never rewrites it (Morticia 5Q-1).
	fn applied_to(&self, base: &Config) -> Config {
		let mut next = base.clone();
		next.roots = self.roots.iter().map(|r| PathBuf::from(r.trim())).collect();
		next.exclude = self.exclude.iter().map(|e| e.trim().to_string()).collect();
		next.depth = self.depth;
		next.theme = self.theme;
		next.commands.editor = self.editor.trim().to_string();
		next.commands.terminal = self.terminal.trim().to_string();
		next
	}
}

impl AppState {
	/// Opens the settings screen on a copy of the config in use: config.toml as this session
	/// loaded it, with any pin changes made since.
	pub fn open_settings(&mut self) {
		self.settings = Some(SettingsDraft::from_config(&self.config));
	}

	/// Drops the draft; the config is as it was (settings-screen clause 4).
	pub fn close_settings(&mut self) {
		self.settings = None;
	}

	/// Saves the draft to the config file this session loaded, then applies it: the theme at once,
	/// and a rescan when the roots, excludes or depth changed (settings-screen clause 3). A failed
	/// save keeps the screen open with its error and changes nothing. Without a config path (the
	/// UI tests' default) the change lasts for the session only, as a pin's does.
	pub fn save_settings(&mut self, ctx: &egui::Context) {
		let Some(draft) = &self.settings else {
			return;
		};
		if !draft.ready() {
			return;
		}
		let next = draft.applied_to(&self.config);
		if let Some(file) = &self.config_path
			&& let Err(e) = repose_core::settings::save(file, &self.config, &next)
		{
			if let Some(d) = &mut self.settings {
				d.error = Some(format!("The settings were not saved: {e}"));
			}
			return;
		}
		let rescan = next.roots != self.config.roots
			|| next.exclude != self.config.exclude
			|| next.depth != self.config.depth;
		if next.theme != self.config.theme {
			crate::theme::apply(ctx, next.theme);
		}
		self.config = next;
		self.settings = None;
		if rescan {
			self.start_scan(ctx);
		}
	}
}
