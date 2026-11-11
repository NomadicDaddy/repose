//! Local commit activity for the Activity screen (activity-screen): the commit times of every
//! commit reachable from a local branch or a saved remote-tracking branch since a cutoff, and the
//! newest commit time overall. Read offline from refs already on disk; nothing is fetched. HEAD is
//! not named, so a repository with no commit yet reads as empty instead of failing.
//!
//! The cutoff is applied here, not with `git log --since`: git stops walking at the first commit
//! older than `--since`, so a newer commit behind an older-dated one (a rebase that kept author
//! dates, `git am`, a wrong clock) would be missed (Morticia L-1, Roger 5Q-1 on 87ae908). The walk
//! is bounded by `MAX_WALK` commits instead, newest first.

use std::collections::HashSet;
use std::path::Path;

use super::commands::{GitError, Verb, run};

/// The most commits one read walks. Far more than any repository makes in the grid's days, and it
/// keeps a read of a very long history short.
const MAX_WALK: usize = 20_000;

/// A repository's commit activity: the commit times (seconds since the Unix epoch) at or after the
/// cutoff, each commit once, and the newest commit time on any branch, which may be older.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Activity {
	pub times: Vec<i64>,
	pub newest: Option<i64>,
}

/// Reads `repo`'s activity since `since` (seconds since the Unix epoch).
pub fn read_activity(repo: &Path, since: i64) -> Result<Activity, GitError> {
	let max = format!("--max-count={MAX_WALK}");
	let args = [
		"--format=%H %ct",
		max.as_str(),
		"--branches",
		"--remotes",
		"--",
	];
	let walked = parse_times(&run(repo, Verb::Log, &args)?);
	Ok(Activity {
		newest: walked.iter().copied().max(),
		times: walked.into_iter().filter(|t| *t >= since).collect(),
	})
}

/// One `<hash> <commit time>` per line; each commit counted once even if listed twice.
pub fn parse_times(bytes: &[u8]) -> Vec<i64> {
	let text = String::from_utf8_lossy(bytes);
	let mut seen = HashSet::new();
	let mut out = Vec::new();
	for line in text.lines() {
		let mut parts = line.split_whitespace();
		let (Some(hash), Some(time)) = (parts.next(), parts.next()) else {
			continue;
		};
		if let Ok(t) = time.parse::<i64>()
			&& seen.insert(hash.to_string())
		{
			out.push(t);
		}
	}
	out
}

#[cfg(test)]
mod tests {
	use super::parse_times;

	#[test]
	fn each_commit_counts_once_and_bad_lines_are_skipped() {
		let input = b"aaa 100\nbbb 200\naaa 100\nnot-a-line\nccc x\n\nddd 50\n";
		assert_eq!(parse_times(input), vec![100, 200, 50]);
	}
}
