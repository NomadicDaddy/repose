//! What a git spawn can fail with, said in the user's terms.

use std::time::Duration;

#[derive(Debug, Clone)]
pub enum GitError {
	NotFound,
	Spawn(String),
	TimedOut {
		after: Duration,
		stderr_tail: String,
	},
	Failed {
		code: Option<i32>,
		stderr_tail: String,
	},
	/// git exited but its output never arrived in time, usually because a process it started is
	/// still holding the pipe. A read must not be mistaken for an empty result.
	OutputLost,
	/// git reported success but the paths it was asked to remove are still there, as `git clean`
	/// does for a nested repository. Success must not be claimed for them.
	StillPresent(Vec<String>),
	/// HEAD names a branch whose ref file exists but does not resolve: a truncated or corrupt
	/// ref, not a branch with no commits yet. No write may treat it as unborn.
	UnreadableRef(String),
	/// The user cancelled a write: git was killed, and its process tree too unless `tree_killed`
	/// is false, in which case the hook's children may still be running.
	Cancelled {
		stderr_tail: String,
		tree_killed: bool,
	},
	/// The user cancelled a commit and git would not say afterwards whether it had landed (a
	/// HEAD read failed); neither "committed" nor "cancelled" may be claimed (Roger 5Q-1 on
	/// 28883f9).
	OutcomeUnknown {
		tree_killed: bool,
	},
}

impl std::fmt::Display for GitError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::UnreadableRef(r) if r == "HEAD" => write!(
				f,
				"HEAD does not resolve to a commit and git cannot name its branch; a ref file may be truncated. Nothing was changed."
			),
			Self::UnreadableRef(r) => write!(
				f,
				"HEAD points at {r}, which exists but does not resolve to a commit; the ref file may be truncated. Nothing was changed."
			),
			Self::StillPresent(paths) => write!(
				f,
				"git clean left {} in place (a nested repository is not removed): {}",
				if paths.len() == 1 {
					"this path"
				} else {
					"these paths"
				},
				paths.join(", ")
			),
			Self::NotFound => write!(f, "git was not found on PATH"),
			Self::Spawn(e) => write!(f, "git could not start: {e}"),
			Self::TimedOut { after, stderr_tail } => {
				write!(f, "git timed out after {}s: {stderr_tail}", after.as_secs())
			}
			Self::Failed { code, stderr_tail } => {
				write!(
					f,
					"git exited {}: {stderr_tail}",
					code.map_or("?".into(), |c| c.to_string())
				)
			}
			Self::OutputLost => write!(f, "git exited but its output was not collected"),
			Self::OutcomeUnknown { tree_killed } => write!(
				f,
				"cancelled while a hook ran, and git would not say afterwards whether the commit had landed; check History before committing again{}",
				if *tree_killed {
					""
				} else {
					" (stopping the hook's processes failed too, so they may still be running)"
				}
			),
			Self::Cancelled {
				stderr_tail,
				tree_killed: false,
			} => write!(
				f,
				"cancelled; git was stopped, but stopping its hooks' processes failed, so they may still be running{}{stderr_tail}",
				if stderr_tail.is_empty() { "" } else { ": " }
			),
			Self::Cancelled { stderr_tail, .. } if stderr_tail.is_empty() => {
				write!(f, "cancelled; git and its hooks were stopped")
			}
			Self::Cancelled { stderr_tail, .. } => {
				write!(
					f,
					"cancelled; git and its hooks were stopped: {stderr_tail}"
				)
			}
		}
	}
}
