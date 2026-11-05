//! Windows' "Animation effects" setting (Settings > Accessibility > Visual effects), read once at
//! launch. `std` has no binding for SystemParametersInfo and the crate forbids unsafe, so a hidden
//! pwsh asks for SPI_GETCLIENTAREAANIMATION through Add-Type, as the clock asks `w32tm` (Phillip
//! #35904, choice A). It takes about a second, so callers run it off the UI thread.

use std::process::{Command, Stdio};

/// SPI_GETCLIENTAREAANIMATION, read into a BOOL. Prints True, False, or unknown when the call
/// fails.
const QUERY: &str = "Add-Type -Namespace Repose -Name Motion -MemberDefinition '[DllImport(\"user32.dll\")] public static extern bool SystemParametersInfo(uint action, uint param, ref bool value, uint flags);'; $on = $false; if ([Repose.Motion]::SystemParametersInfo(0x1042, 0, [ref] $on, 0)) { $on } else { 'unknown' }";

/// No console window, and below-normal priority so pwsh's start-up and C# compile yield the CPU
/// to repose drawing and reading (the a3b9a3a gate's startup and diff load slowed by ~300-480 ms
/// with the query at normal priority during launch).
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;

/// Whether the user leaves animation effects on, or None when it could not be read.
pub fn animations_enabled() -> Option<bool> {
	use std::os::windows::process::CommandExt;
	let mut cmd = Command::new("pwsh");
	cmd.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
	let out = cmd
		.args(["-NoProfile", "-NonInteractive", "-Command", QUERY])
		.stdin(Stdio::null())
		.stderr(Stdio::null())
		.output()
		.ok()?;
	parse(&String::from_utf8_lossy(&out.stdout))
}

fn parse(text: &str) -> Option<bool> {
	match text.trim() {
		"True" => Some(true),
		"False" => Some(false),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::{animations_enabled, parse};

	/// The live query on this desk: pwsh answers True or False, never unknown. Ignored because it
	/// starts pwsh; run with `cargo test -p repose -- --ignored reads_the_desktop`.
	#[test]
	#[ignore = "starts pwsh and reads this desk's setting"]
	fn reads_the_desktop_setting() {
		assert!(animations_enabled().is_some());
	}

	#[test]
	fn the_answer_is_true_false_or_unknown() {
		assert_eq!(parse("True\r\n"), Some(true));
		assert_eq!(parse("False\n"), Some(false));
		assert_eq!(parse("unknown\n"), None);
		assert_eq!(parse(""), None, "pwsh missing or killed");
	}
}
