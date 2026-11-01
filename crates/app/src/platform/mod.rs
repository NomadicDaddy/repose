//! OS-specific actions behind one trait, so a port is a module.

use std::path::Path;

#[cfg(windows)]
mod windows;

pub trait Platform {
	fn open_folder(&self, dir: &Path) -> Result<(), String>;
	/// Opens the file's folder with the file selected.
	fn reveal_file(&self, file: &Path) -> Result<(), String>;
	/// Opens a working-tree file at a line, or a repository folder when `line` is None, in the
	/// configured editor (`commands.editor`, by default `code`). An editor that is not on PATH is
	/// an error naming the command, never a silent no-op.
	fn open_in_editor(
		&self,
		command: &str,
		target: &Path,
		line: Option<usize>,
	) -> Result<(), String>;
	/// Opens a shell in the folder: the configured terminal (`commands.terminal`, by default
	/// `wt`) with pwsh in that directory, or pwsh alone when the terminal is not on PATH.
	fn open_terminal(&self, command: &str, dir: &Path) -> Result<(), String>;
}

pub fn current() -> Box<dyn Platform> {
	#[cfg(windows)]
	{
		Box::new(windows::Windows)
	}
	#[cfg(not(windows))]
	{
		Box::new(Unsupported)
	}
}

#[cfg(not(windows))]
struct Unsupported;

#[cfg(not(windows))]
impl Platform for Unsupported {
	fn open_folder(&self, _dir: &Path) -> Result<(), String> {
		Err("opening folders is implemented on Windows only".into())
	}
	fn reveal_file(&self, _file: &Path) -> Result<(), String> {
		Err("revealing files is implemented on Windows only".into())
	}
	fn open_in_editor(
		&self,
		_command: &str,
		_target: &Path,
		_line: Option<usize>,
	) -> Result<(), String> {
		Err("opening an editor is implemented on Windows only".into())
	}
	fn open_terminal(&self, _command: &str, _dir: &Path) -> Result<(), String> {
		Err("opening a terminal is implemented on Windows only".into())
	}
}

/// Minutes east of UTC for the machine's current local time, or None when it cannot be read,
/// in which case callers show UTC and say so.
pub fn utc_offset_minutes() -> Option<i32> {
	#[cfg(windows)]
	{
		windows::utc_offset_minutes()
	}
	#[cfg(not(windows))]
	{
		None
	}
}
