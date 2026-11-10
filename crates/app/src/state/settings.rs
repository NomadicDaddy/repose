//! The settings screen's draft (settings-screen clauses 1 to 4): the config's roots, excludes,
//! depth, theme and commands copied when the screen opens, edited there, checked as they are
//! typed, and either saved to config.toml and applied at once (Save) or dropped (Cancel, Escape).

use std::path::{Path, PathBuf};

use eframe::egui;
use repose_core::config::{self, Config, Theme};

use super::AppState;

/// The deepest scan the depth control offers; deeper walks get slow on large drives.
pub const MAX_DEPTH: u32 = 8;

pub struct SettingsDraft {
	pub roots: Vec<String>,
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
		Self {
			roots: config
				.roots
				.iter()
				.map(|r| r.display().to_string())
				.collect(),
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

	/// What is wrong with a root, for the line under it: nothing for a folder that exists now.
	pub fn root_problem(root: &str) -> Option<&'static str> {
		(!Path::new(root.trim()).is_dir()).then_some("This folder does not exist.")
	}

	/// What is wrong with a command, for the line under its box.
	pub fn command_problem(command: &str) -> Option<&'static str> {
		(!config::command_ok(command.trim()))
			.then_some("Use a program name or an absolute path, with no spaces or arguments.")
	}

	/// What is wrong with an excluded name: it is a folder name, never a path.
	pub fn exclude_problem(name: &str) -> Option<&'static str> {
		let name = name.trim();
		(name.is_empty() || name.contains(['/', '\\']))
			.then_some("Use a folder name, without slashes.")
	}

	/// Save is offered only when nothing is marked (settings-screen clause 2).
	pub fn ready(&self) -> bool {
		self.roots.iter().all(|r| Self::root_problem(r).is_none())
			&& self
				.exclude
				.iter()
				.all(|e| Self::exclude_problem(e).is_none())
			&& Self::command_problem(&self.editor).is_none()
			&& Self::command_problem(&self.terminal).is_none()
	}

	/// `base` with this draft's settings in place of its own; the pins and anything else stay.
	fn applied_to(&self, base: &Config) -> Config {
		let mut next = base.clone();
		next.roots = self.roots.iter().map(|r| PathBuf::from(r.trim())).collect();
		next.exclude = self.exclude.iter().map(|e| e.trim().to_string()).collect();
		next.depth = self.depth.clamp(1, MAX_DEPTH);
		next.theme = self.theme;
		next.commands.editor = self.editor.trim().to_string();
		next.commands.terminal = self.terminal.trim().to_string();
		next
	}
}

impl AppState {
	/// Opens the settings screen on a copy of the config in use.
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
