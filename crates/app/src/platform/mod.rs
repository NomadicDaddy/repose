//! OS-specific actions behind one trait, so a port is a module.

use std::path::Path;

#[cfg(windows)]
mod windows;

pub trait Platform {
	fn open_folder(&self, dir: &Path) -> Result<(), String>;
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
}

/// Minutes east of UTC for the machine's current local time, without a time-zone crate.
pub fn utc_offset_minutes() -> i32 {
	#[cfg(windows)]
	{
		windows::utc_offset_minutes()
	}
	#[cfg(not(windows))]
	{
		0
	}
}
