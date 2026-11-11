//! activity-screen clause 5: commit activity read from local and remote-tracking branches, each
//! commit once, with no fetch and no failure on a repository that has no commit yet.

mod support;

use std::time::{SystemTime, UNIX_EPOCH};

use repose_core::git::activity::read_activity;
use support::TempRepo;

fn now() -> i64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|d| d.as_secs() as i64)
		.unwrap_or(0)
}

#[test]
fn a_repository_with_no_commit_reads_as_empty() {
	let repo = TempRepo::new("activity-empty");
	let activity = read_activity(&repo.path, 0).unwrap_or_else(|e| panic!("{e}"));
	assert!(activity.times.is_empty());
	assert_eq!(activity.newest, None);
}

#[test]
fn commits_on_every_branch_count_once_each() {
	let repo = TempRepo::new("activity-branches");
	repo.write("a.txt", "one\n");
	repo.commit_all("first");
	repo.git(&["checkout", "-q", "-b", "side"]);
	repo.write("b.txt", "two\n");
	repo.commit_all("on side");
	repo.git(&["checkout", "-q", "-"]);
	repo.write("c.txt", "three\n");
	repo.commit_all("on main");
	let activity = read_activity(&repo.path, now() - 3600).unwrap_or_else(|e| panic!("{e}"));
	assert_eq!(
		activity.times.len(),
		3,
		"the shared first commit is counted once, plus one commit on each branch"
	);
	let newest = activity
		.newest
		.unwrap_or_else(|| panic!("a newest commit time"));
	assert!(activity.times.iter().all(|t| *t <= newest));
}

#[test]
fn a_cutoff_after_every_commit_still_reports_the_newest() {
	let repo = TempRepo::new("activity-cutoff");
	repo.write("a.txt", "one\n");
	repo.commit_all("first");
	let activity = read_activity(&repo.path, now() + 3600).unwrap_or_else(|e| panic!("{e}"));
	assert!(activity.times.is_empty(), "nothing at or after the cutoff");
	assert!(
		activity.newest.is_some(),
		"the quiet tile still needs the newest commit time"
	);
}
