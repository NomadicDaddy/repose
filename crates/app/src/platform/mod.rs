//! OS-specific actions behind one trait, so a port is a module.

use std::path::Path;

#[cfg(windows)]
mod windows;

pub trait Platform {
	fn open_folder(&self, dir: &Path) -> Result<(), String>;
	/// Opens the file's folder with the file selected.
	fn reveal_file(&self, file: &Path) -> Result<(), String>;
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
