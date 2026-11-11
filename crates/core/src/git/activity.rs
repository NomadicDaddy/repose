//! Local commit activity for the Activity screen (activity-screen): the commit times of every
//! commit reachable from a local branch or a saved remote-tracking branch since a cutoff, and the
//! newest commit time overall. Read offline from refs already on disk; nothing is fetched. HEAD is
//! not named, so a repository with no commit yet reads as empty instead of failing.

use std::collections::HashSet;
use std::path::Path;

use super::commands::{GitError, Verb, run};

/// A repository's commit activity: the commit times (seconds since the Unix epoch) at or after the
/// cutoff, each commit once, and the newest commit time on any branch, which may be older.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Activity {
	pub times: Vec<i64>,
	pub newest: Option<i64>,
}

/// Reads `repo`'s activity since `since` (seconds since the Unix epoch).
pub fn read_activity(repo: &Path, since: i64) -> Result<Activity, GitError> {
	let since_arg = format!("--since=@{since}");
	let recent = run(
		repo,
		Verb::Log,
		&[
			"--format=%H %ct",
			&since_arg,
			"--branches",
			"--remotes",
			"--",
		],
	)?;
	let newest = run(
		repo,
		Verb::Log,
		&["-1", "--format=%H %ct", "--branches", "--remotes", "--"],
	)?;
	Ok(Activity {
		times: parse_times(&recent),
		newest: parse_times(&newest).into_iter().max(),
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
