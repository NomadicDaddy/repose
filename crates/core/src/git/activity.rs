//! Local commit activity for the Activity screen (activity-screen): every commit reachable from a
//! local branch or a saved remote-tracking branch since a cutoff, with its time and its local
//! date, and the newest commit time overall. Read offline from refs already on disk; nothing is
//! fetched. HEAD is not named, so a repository with no commit yet reads as empty instead of failing.
//!
//! The cutoff is applied here, not with `git log --since`: git stops walking at the first commit
//! older than `--since`, so a newer commit behind an older-dated one (a rebase that kept author
//! dates, `git am`, a wrong clock) would be missed (Morticia L-1, Roger 5Q-1 on 87ae908). The walk
//! is bounded by `MAX_WALK` commits instead, newest first, and a read that reaches the bound says
//! so (`truncated`), so the screen never shows a cut count as complete (Roger 5Q-1, Morticia on
//! 4431bdb).
//!
//! Each commit's local date comes from git's `format-local`, which converts it with the zone rule
//! in force at that instant, so a commit from before a daylight-saving change lands on its own
//! local day (Roger 5Q-3, Morticia L-1 on 4051085); one current offset for every day would not.

use std::collections::HashSet;
use std::path::Path;

use super::commands::{GitError, Verb, run};

/// The most commits one read walks, which keeps a read of a very long history short.
pub const MAX_WALK: usize = 20_000;

/// One commit: its committer time (seconds since the Unix epoch) and its local date, as days since
/// 1970-01-01.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitDay {
	pub time: i64,
	pub local_day: i64,
}

/// A repository's commit activity: the commits at or after the cutoff, each once, the newest
/// commit time on any branch, which may be older, and whether the walk stopped at `MAX_WALK`, when
/// older commits in the cutoff's days may be missing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Activity {
	pub commits: Vec<CommitDay>,
	pub newest: Option<i64>,
	pub truncated: bool,
}

/// Reads `repo`'s activity since `since` (seconds since the Unix epoch).
pub fn read_activity(repo: &Path, since: i64) -> Result<Activity, GitError> {
	let max = format!("--max-count={MAX_WALK}");
	let args = [
		"--format=%H %ct %cd",
		"--date=format-local:%Y-%m-%d",
		max.as_str(),
		"--branches",
		"--remotes",
		"--",
	];
	let walked = parse_commits(&run(repo, Verb::Log, &args)?);
	Ok(Activity {
		truncated: walked.len() >= MAX_WALK,
		newest: walked.iter().map(|c| c.time).max(),
		commits: walked.into_iter().filter(|c| c.time >= since).collect(),
	})
}

/// One `<hash> <commit time> <YYYY-MM-DD>` per line; each commit counted once even if listed
/// twice, and a line that does not parse skipped.
pub fn parse_commits(bytes: &[u8]) -> Vec<CommitDay> {
	let text = String::from_utf8_lossy(bytes);
	let mut seen = HashSet::new();
	let mut out = Vec::new();
	for line in text.lines() {
		let mut parts = line.split_whitespace();
		let (Some(hash), Some(time), Some(date)) = (parts.next(), parts.next(), parts.next())
		else {
			continue;
		};
		if let (Ok(time), Some(local_day)) = (time.parse::<i64>(), days_from_date(date))
			&& seen.insert(hash.to_string())
		{
			out.push(CommitDay { time, local_day });
		}
	}
	out
}

/// `YYYY-MM-DD` as days since 1970-01-01 (Howard Hinnant's days_from_civil).
fn days_from_date(date: &str) -> Option<i64> {
	let mut parts = date.splitn(3, '-').map(|p| p.parse::<i64>().ok());
	let (Some(Some(y)), Some(Some(m)), Some(Some(d))) = (parts.next(), parts.next(), parts.next())
	else {
		return None;
	};
	if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
		return None;
	}
	let y = if m <= 2 { y - 1 } else { y };
	let era = y.div_euclid(400);
	let yoe = y - era * 400;
	let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	Some(era * 146_097 + doe - 719_468)
}

#[cfg(test)]
mod tests {
	use super::{CommitDay, days_from_date, parse_commits};

	#[test]
	fn each_commit_counts_once_and_bad_lines_are_skipped() {
		let input = b"aaa 100 1970-01-01\nbbb 200 1970-01-02\naaa 100 1970-01-01\nnot-a-line\nccc x 1970-01-01\nddd 5 1970-13-01\n\neee 50 1969-12-31\n";
		let day = |time, local_day| CommitDay { time, local_day };
		assert_eq!(
			parse_commits(input),
			vec![day(100, 0), day(200, 1), day(50, -1)]
		);
	}

	#[test]
	fn dates_become_day_numbers() {
		assert_eq!(days_from_date("1970-01-01"), Some(0));
		assert_eq!(days_from_date("2026-11-11"), Some(20_768));
		assert_eq!(days_from_date("2024-02-29"), Some(19_782));
		assert_eq!(days_from_date("2026-11"), None);
	}
}
