//! remote-pull-push: Push, the checked-out branch to the upstream it tracks and nothing else (A1
//! as revised, Phillip #58501). It names its remote and refspec, so no push, push-remote, mirror
//! or push.default setting can widen it, and a lease on the commit last seen there keeps a
//! deleted or moved server branch as it is. The remotes are local repositories, so nothing
//! leaves the machine.
#![allow(clippy::expect_used)]

mod support;

use std::sync::atomic::Ordering;
use std::time::Duration;

use repose_core::git::remote::{plan_push, push, send_push};
use repose_core::git::{GitError, read_status};
use support::{TempRepo, upstream_and_clone, watchdog};

fn push_now(clone: &TempRepo) -> Result<(), GitError> {
	let (cancel, done) = watchdog(Duration::from_secs(60));
	let result = push(&clone.path, Some(&cancel));
	done.store(true, Ordering::SeqCst);
	result
}

/// The upstream's checked-out branch takes pushes and moves its own work tree with them.
fn accepts_pushes(upstream: &TempRepo) {
	upstream.git(&["config", "receive.denyCurrentBranch", "updateInstead"]);
}

#[test]
fn push_sends_the_branch_to_its_upstream() {
	let (upstream, clone) = upstream_and_clone("push-ok");
	accepts_pushes(&upstream);
	clone.write("b.txt", "2\n");
	clone.commit_all("two");
	let result = push_now(&clone);
	assert!(result.is_ok(), "push failed: {result:?}");
	assert_eq!(
		upstream.git(&["rev-parse", "main"]),
		clone.git(&["rev-parse", "HEAD"])
	);
	assert_eq!(read_status(&clone.path).expect("status").ahead, Some(0));
}

#[test]
fn a_rejected_push_names_gits_error_and_changes_nothing_local() {
	let (upstream, clone) = upstream_and_clone("push-rejected");
	accepts_pushes(&upstream);
	upstream.write("a.txt", "theirs\n");
	upstream.commit_all("theirs");
	clone.write("a.txt", "mine\n");
	clone.commit_all("mine");
	let refs = clone.git(&["for-each-ref"]);
	let theirs = upstream.git(&["rev-parse", "main"]);
	match push_now(&clone) {
		Err(GitError::Failed { stderr_tail, .. }) => assert!(
			stderr_tail.contains("rejected"),
			"git's own rejection is the message: {stderr_tail:?}"
		),
		other => panic!("a rejected push must fail with git's message, got {other:?}"),
	}
	assert_eq!(clone.git(&["for-each-ref"]), refs, "no local ref moved");
	assert_eq!(
		upstream.git(&["rev-parse", "main"]),
		theirs,
		"nothing was forced"
	);
}

/// A clone checked out on main with new work on main and on a tracked `other` branch, and an
/// annotated tag; the upstream takes pushes. Returns both and the upstream's `other` before.
fn two_branches_and_a_tag(label: &str) -> (TempRepo, TempRepo, String) {
	let (upstream, clone) = upstream_and_clone(label);
	accepts_pushes(&upstream);
	upstream.git(&["branch", "other"]);
	clone.git(&["fetch", "-q"]);
	clone.git(&["branch", "--track", "other", "origin/other"]);
	clone.git(&["switch", "-q", "other"]);
	clone.write("o.txt", "o\n");
	clone.commit_all("other work");
	clone.git(&["switch", "-q", "main"]);
	clone.write("m.txt", "m\n");
	clone.commit_all("main work");
	clone.git(&["tag", "-a", "v9", "-m", "a tag"]);
	let other = upstream.git(&["rev-parse", "other"]);
	(upstream, clone, other)
}

/// Each key would otherwise send more than the user's one branch, or force it (Roger 5Q-1 and
/// Morticia M on f61983d): the explicit remote and HEAD refspec override every one.
#[test]
fn push_sends_only_the_checked_out_branch_whatever_the_config_says() {
	for (key, value) in [
		("push.default", "matching"),
		("push.followTags", "true"),
		("remote.origin.push", "refs/heads/*:refs/heads/*"),
		("remote.origin.push", "+refs/heads/other:refs/heads/other"),
	] {
		let (upstream, clone, other) = two_branches_and_a_tag("push-only-current");
		clone.git(&["config", key, value]);
		let result = push_now(&clone);
		assert!(result.is_ok(), "{key}={value}: push failed: {result:?}");
		assert_eq!(
			upstream.git(&["rev-parse", "main"]),
			clone.git(&["rev-parse", "main"]),
			"{key}={value}: main went"
		);
		assert_eq!(
			upstream.git(&["rev-parse", "other"]),
			other,
			"{key}={value}: other stayed"
		);
		assert_eq!(
			upstream.git(&["tag", "--list"]),
			"",
			"{key}={value}: no tag went"
		);
	}
}

/// A push remote is not the upstream: the branch goes where it tracks, never to another remote.
#[test]
fn push_goes_to_the_upstream_remote_not_a_push_remote() {
	for key in ["branch.main.pushRemote", "remote.pushDefault"] {
		let (upstream, clone, _) = two_branches_and_a_tag("push-remote");
		let elsewhere = TempRepo::new("push-elsewhere");
		accepts_pushes(&elsewhere);
		let path = elsewhere.path.to_string_lossy().replace('\\', "/");
		clone.git(&["remote", "add", "elsewhere", &path]);
		clone.git(&["config", key, "elsewhere"]);
		let result = push_now(&clone);
		assert!(result.is_ok(), "{key}: push failed: {result:?}");
		assert_eq!(
			upstream.git(&["rev-parse", "main"]),
			clone.git(&["rev-parse", "main"]),
			"{key}: the upstream got the branch"
		);
		assert_eq!(
			elsewhere.git(&["for-each-ref"]),
			"",
			"{key}: nothing went elsewhere"
		);
	}
}

/// A mirror remote would delete what the clone lacks; with a refspec git refuses instead.
#[test]
fn a_mirror_remote_is_refused_and_nothing_is_deleted() {
	let (upstream, clone, other) = two_branches_and_a_tag("push-mirror");
	upstream.git(&["branch", "only-there"]);
	clone.git(&["config", "remote.origin.mirror", "true"]);
	let main = upstream.git(&["rev-parse", "main"]);
	match push_now(&clone) {
		Err(GitError::Failed { stderr_tail, .. }) => assert!(
			stderr_tail.contains("mirror"),
			"git's own refusal is the message: {stderr_tail:?}"
		),
		other => panic!("a mirror push must be refused, got {other:?}"),
	}
	assert!(
		!upstream.git(&["branch", "--list", "only-there"]).is_empty(),
		"nothing deleted"
	);
	assert_eq!(upstream.git(&["rev-parse", "main"]), main, "main unchanged");
	assert_eq!(
		upstream.git(&["rev-parse", "other"]),
		other,
		"other unchanged"
	);
}

#[test]
fn a_detached_head_or_a_branch_with_no_upstream_is_not_pushed() {
	let (_upstream, clone) = upstream_and_clone("push-no-branch");
	clone.git(&["switch", "-q", "--detach"]);
	assert!(
		matches!(push_now(&clone), Err(GitError::Spawn(_))),
		"detached HEAD"
	);
	clone.git(&["switch", "-q", "-c", "local-only"]);
	assert!(
		matches!(push_now(&clone), Err(GitError::Spawn(_))),
		"no upstream"
	);
}

/// A branch deleted on the server since the last fetch still has its saved tracking ref; the
/// lease refuses to create it again (Roger 5Q-3 on 2d1efdd). One someone else moved since is not
/// overwritten either, and a branch behind its upstream is refused before anything is sent.
#[test]
fn push_never_recreates_a_deleted_branch_or_overwrites_a_moved_one() {
	let (upstream, clone, _) = two_branches_and_a_tag("push-stale");
	upstream.git(&["switch", "-q", "other"]);
	upstream.git(&["branch", "-D", "main"]);
	match push_now(&clone) {
		Err(GitError::Failed { stderr_tail, .. }) => assert!(
			stderr_tail.contains("stale info"),
			"git's own refusal is the message: {stderr_tail:?}"
		),
		other => panic!("a deleted upstream must not be recreated, got {other:?}"),
	}
	assert_eq!(
		upstream.git(&["branch", "--list", "main"]),
		"",
		"main stays deleted"
	);

	let (upstream, clone, _) = two_branches_and_a_tag("push-moved");
	upstream.write(
		"t.txt", "theirs
",
	);
	upstream.commit_all("theirs, not yet fetched");
	let theirs = upstream.git(&["rev-parse", "main"]);
	assert!(push_now(&clone).is_err(), "a moved upstream is refused");
	assert_eq!(
		upstream.git(&["rev-parse", "main"]),
		theirs,
		"nothing was overwritten"
	);

	let (upstream, clone) = upstream_and_clone("push-behind");
	upstream.write("b.txt", "2\n");
	upstream.commit_all("theirs");
	clone.git(&["fetch", "-q"]);
	assert!(
		matches!(push_now(&clone), Err(GitError::Spawn(_))),
		"behind: nothing sent"
	);
}

/// The commit sent is the one plan checked, not HEAD read again: a checkout or commit between the
/// two cannot change what goes, so the lease never carries a non-fast-forward (Roger 5Q-4 on
/// c50d643).
#[test]
fn push_sends_the_planned_commit_even_if_the_branch_moves_before_sending() {
	let (upstream, clone) = upstream_and_clone("push-pinned");
	accepts_pushes(&upstream);
	clone.write("b.txt", "2\n");
	clone.commit_all("planned");
	let plan = plan_push(&clone.path).expect("a plan");
	assert_eq!(plan.source(), clone.git(&["rev-parse", "HEAD"]).trim());
	// Another tool rewrites the branch between plan and send.
	clone.git(&["reset", "-q", "--hard", "HEAD~1"]);
	clone.write("c.txt", "3\n");
	clone.commit_all("not planned");
	let moved = clone.git(&["rev-parse", "HEAD"]);
	let (cancel, done) = watchdog(Duration::from_secs(60));
	let result = send_push(&clone.path, &plan, Some(&cancel));
	done.store(true, Ordering::SeqCst);
	assert!(result.is_ok(), "push failed: {result:?}");
	let sent = upstream.git(&["rev-parse", "main"]);
	assert_eq!(sent.trim(), plan.source(), "the planned commit went");
	assert_ne!(sent, moved, "not the commit HEAD moved to");
}
