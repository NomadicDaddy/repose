//! Stopping git with everything it started, for the user's Cancel on a write.

use std::process::{Command, Stdio};

use super::commands::hide_console;

/// Kills git and everything under it. Killing git alone would leave a hook and the hook's
/// children running on; on Windows `taskkill /T` takes the tree, and the child is reaped after.
/// Returns whether the tree kill itself succeeded; git is reaped either way, and a false answer
/// is reported to the user rather than claimed as stopped (Roger 5Q-2 on a31e63b).
pub(super) fn kill_tree(child: &mut std::process::Child) -> bool {
	#[cfg(windows)]
	let tree_killed = {
		let mut kill = Command::new("taskkill");
		hide_console(&mut kill);
		kill.args(["/PID", &child.id().to_string(), "/T", "/F"])
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status()
			.is_ok_and(|s| s.success())
	};
	#[cfg(not(windows))]
	let tree_killed = child.kill().is_ok();
	let _ = child.kill();
	let _ = child.wait();
	tree_killed
}
