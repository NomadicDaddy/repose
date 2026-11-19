//! remote-pull-push: Pull and Push, the user's two remote writes beside Fetch (A1 as revised,
//! Phillip #58496, #58500, #58501). Pull only fast-forwards, so a diverged branch is refused
//! with the branch, index and working tree unchanged and no merge or rebase left behind, whatever
//! pull.rebase says. Here too: the arguments both refuse, and a sign-in refused for both. Push's
//! own scope is in tests/push.rs. The remotes are local repositories, so nothing leaves the
//! machine.
#![allow(clippy::expect_used)]

mod support;

use std::sync::atomic::Ordering;
use std::time::Duration;

use repose_core::git::GitError;
use repose_core::git::Verb;
use repose_core::git::commands::run;
use repose_core::git::remote::{pull, push};
use support::{TempRepo, sign_in_listener, upstream_and_clone, watchdog};

fn pull_now(clone: &TempRepo) -> Result<(), GitError> {
	let (cancel, done) = watchdog(Duration::from_secs(60));
	let result = pull(&clone.path, Some(&cancel));
	done.store(true, Ordering::SeqCst);
	result
}

fn push_now(clone: &TempRepo) -> Result<(), GitError> {
	let (cancel, done) = watchdog(Duration::from_secs(60));
	let result = push(&clone.path, Some(&cancel));
	done.store(true, Ordering::SeqCst);
	result
}

#[test]
fn pull_fast_forwards_to_the_upstreams_commits() {
	let (upstream, clone) = upstream_and_clone("pull-ok");
	upstream.write("b.txt", "2\n");
	upstream.commit_all("two");
	let result = pull_now(&clone);
	assert!(result.is_ok(), "pull failed: {result:?}");
	assert_eq!(
		clone.git(&["rev-parse", "HEAD"]),
		upstream.git(&["rev-parse", "HEAD"])
	);
	assert!(clone.path.join("b.txt").exists(), "the work tree moved too");
}

#[test]
fn a_diverged_pull_is_refused_and_changes_nothing() {
	let (upstream, clone) = upstream_and_clone("pull-diverged");
	upstream.write("a.txt", "theirs\n");
	upstream.commit_all("theirs");
	clone.write("a.txt", "mine\n");
	clone.commit_all("mine");
	// A configured rebase or merge preference must not turn the refusal into a half-done rebase.
	clone.git(&["config", "pull.rebase", "true"]);
	let head = clone.git(&["rev-parse", "HEAD"]);
	match pull_now(&clone) {
		Err(GitError::Failed { stderr_tail, .. }) => assert!(
			stderr_tail.contains("Not possible to fast-forward"),
			"git's own refusal is the message: {stderr_tail:?}"
		),
		other => panic!("a diverged pull must fail with git's message, got {other:?}"),
	}
	assert_eq!(
		clone.git(&["rev-parse", "HEAD"]),
		head,
		"the branch did not move"
	);
	assert_eq!(
		clone.git(&["status", "--porcelain"]),
		"",
		"index and tree unchanged"
	);
	let git_dir = clone.path.join(".git");
	for left in ["MERGE_HEAD", "rebase-merge", "rebase-apply", "AUTO_MERGE"] {
		assert!(!git_dir.join(left).exists(), "{left} was left behind");
	}
	assert_eq!(
		std::fs::read_to_string(clone.path.join("a.txt")).expect("read"),
		"mine\n"
	);
}

/// Pull takes no argument; Push only the lease, remote and HEAD:refs/heads/<branch> that
/// push::push reads from the branch's config, in that order. Nothing else reaches git.
#[test]
fn pull_and_push_refuse_every_other_argument() {
	let (_upstream, clone) = upstream_and_clone("no-extra");
	let sha = "0123456789abcdef0123456789abcdef01234567";
	let lease = format!("--force-with-lease=refs/heads/main:{sha}");
	let other_lease = format!("--force-with-lease=refs/heads/other:{sha}");
	let lease = lease.as_str();
	for (verb, extra) in [
		(Verb::Pull, &["--rebase"][..]),
		(Verb::Pull, &["origin", "other"][..]),
		(Verb::Push, &[][..]),
		(Verb::Push, &["--force"][..]),
		(Verb::Push, &["--tags"][..]),
		(Verb::Push, &["origin", "HEAD:refs/heads/main"][..]),
		(
			Verb::Push,
			&["--force", "origin", "HEAD:refs/heads/main"][..],
		),
		(
			Verb::Push,
			&["--force-with-lease", "origin", "HEAD:refs/heads/main"][..],
		),
		(
			Verb::Push,
			&[other_lease.as_str(), "origin", "HEAD:refs/heads/main"][..],
		),
		(
			Verb::Push,
			&[
				"--force-with-lease=refs/heads/main:zz",
				"origin",
				"HEAD:refs/heads/main",
			][..],
		),
		(Verb::Push, &[lease, "--mirror", "HEAD:refs/heads/main"][..]),
		(Verb::Push, &[lease, "origin", "+HEAD:refs/heads/main"][..]),
		(
			Verb::Push,
			&[lease, "origin", "refs/heads/*:refs/heads/*"][..],
		),
		(Verb::Push, &[lease, "origin", "HEAD:refs/tags/v1"][..]),
		(
			Verb::Push,
			&[lease, "origin", "HEAD:refs/heads/main", "HEAD:refs/heads/x"][..],
		),
	] {
		assert!(
			matches!(run(&clone.path, verb, extra), Err(GitError::Spawn(_))),
			"{verb:?} {extra:?} must never run"
		);
	}
}

/// Like Fetch, a sign-in request on Pull or Push is refused by git or Git Credential Manager, never
/// asked in a window: the 60 s watchdog fails the test instead of letting one hang the gate.
#[test]
fn a_sign_in_request_on_pull_or_push_fails_without_a_prompt_or_window() {
	let port = sign_in_listener();
	let (_upstream, clone) = upstream_and_clone("remote-401");
	let url = format!("http://127.0.0.1:{port}/team/thing.git");
	clone.git(&["remote", "set-url", "origin", &url]);
	let head = clone.git(&["rev-parse", "HEAD"]);
	for (name, result) in [("pull", pull_now(&clone)), ("push", push_now(&clone))] {
		match result {
			Err(GitError::Failed { stderr_tail, .. }) => assert!(
				stderr_tail.contains("interactivity has been disabled")
					|| stderr_tail.contains("terminal prompts disabled"),
				"{name}'s failure is a refusal to prompt: {stderr_tail:?}"
			),
			other => panic!("{name} must fail with git's refusal, got {other:?}"),
		}
	}
	assert_eq!(clone.git(&["rev-parse", "HEAD"]), head, "nothing moved");
}
