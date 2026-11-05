//! OS-specific actions behind one trait, so a port is a module.

use std::path::{Path, PathBuf};

#[cfg(windows)]
mod windows;

/// The working-tree file Git names `git_path` (always forward slashes) inside `repo`. On Windows
/// the slashes become backslashes, so Explorer, the editor and the file system all get one form;
/// every screen that turns a Git path into a file goes through here.
pub fn git_worktree_path(repo: &Path, git_path: &str) -> PathBuf {
	#[cfg(windows)]
	{
		repo.join(git_path.replace('/', "\\"))
	}
	#[cfg(not(windows))]
	{
		repo.join(git_path)
	}
}

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

#[cfg(all(test, windows))]
mod tests {
	use super::*;

	#[test]
	fn a_git_path_joins_the_repository_with_backslashes_only() {
		// Compared as strings: PathBuf equality on Windows treats / and \ as the same separator,
		// so it could not see a slash left in (Roger Info on 4a95181).
		let full = git_worktree_path(Path::new("C:/roots/x\\repo"), "src/a b/c.rs");
		assert_eq!(full.to_str(), Some("C:/roots/x\\repo\\src\\a b\\c.rs"));
		let top = git_worktree_path(Path::new("C:\\r"), "top.txt");
		assert_eq!(top.to_str(), Some("C:\\r\\top.txt"));
		// Control: a plain join keeps the slash, and this comparison sees it.
		let plain = Path::new("C:\\r").join("a/b.txt");
		assert_ne!(plain.to_str(), Some("C:\\r\\a\\b.txt"));
		assert_eq!(
			plain,
			PathBuf::from("C:\\r\\a\\b.txt"),
			"PathBuf equality cannot tell"
		);
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
