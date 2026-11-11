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

/// One `git fast-import` commit on `branch` with the given committer time, after `from` if any.
fn commit_at(branch: &str, mark: u32, time: i64, from: Option<u32>) -> String {
	let parent = from.map_or(String::new(), |m| format!("from :{m}\n"));
	format!(
		"commit {branch}\nmark :{mark}\ncommitter T <t@example.invalid> {time} +0000\ndata 1\nx\n{parent}\n"
	)
}

/// Morticia L-1 and Roger 5Q-1 on 87ae908: a new commit whose parent is dated 2020 and whose
/// grandparent is new again. `git log --since` stopped at the 2020 commit and missed the
/// grandparent; the cutoff is now applied after the walk, so both new commits count.
#[test]
fn a_new_commit_behind_an_old_dated_one_still_counts() {
	let repo = TempRepo::new("activity-skew");
	let t = now();
	let stream = [
		commit_at("refs/heads/main", 1, t - 60, None),
		commit_at("refs/heads/main", 2, 1_577_836_800, Some(1)),
		commit_at("refs/heads/main", 3, t, Some(2)),
	]
	.concat();
	repo.git_with_input(&["fast-import", "--quiet"], &stream);
	let activity = read_activity(&repo.path, t - 3600).unwrap_or_else(|e| panic!("{e}"));
	let mut times = activity.times.clone();
	times.sort_unstable();
	assert_eq!(times, vec![t - 60, t], "both new commits, not the 2020 one");
	assert_eq!(activity.newest, Some(t));
}

/// Roger 5Q-2 on 87ae908: a commit reachable only from a saved remote-tracking ref counts.
#[test]
fn a_commit_only_on_a_remote_tracking_ref_counts() {
	let repo = TempRepo::new("activity-remote-only");
	let t = now();
	let stream = [
		commit_at("refs/heads/main", 1, t - 120, None),
		commit_at("refs/remotes/origin/feature", 2, t - 60, Some(1)),
	]
	.concat();
	repo.git_with_input(&["fast-import", "--quiet"], &stream);
	let branches = repo.git(&["branch", "--contains", "refs/remotes/origin/feature"]);
	assert!(
		branches.trim().is_empty(),
		"no local branch holds it: {branches}"
	);
	let activity = read_activity(&repo.path, t - 3600).unwrap_or_else(|e| panic!("{e}"));
	assert_eq!(activity.times.len(), 2, "{:?}", activity.times);
	assert_eq!(activity.newest, Some(t - 60));
}
