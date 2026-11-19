//! remote-pull-push: Pull and Push, the user's two remote writes beside Fetch (A1 as revised,
//! Phillip #58496, #58500, #58501). Pull only fast-forwards, so a diverged branch is refused
//! with the branch, index and working tree unchanged and no merge or rebase left behind, whatever
//! pull.rebase says. Push sends only the checked-out branch to the upstream it tracks, with no
//! tags, whatever push.default and push.followTags say. The remotes are local repositories, so
//! nothing leaves the machine.
#![allow(clippy::expect_used)]

mod support;

use std::sync::atomic::Ordering;
use std::time::Duration;

use repose_core::git::GitError;
use repose_core::git::commands::run;
use repose_core::git::remote::{pull, push};
use repose_core::git::{Verb, read_status};
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

/// The upstream's checked-out branch takes pushes and moves its own work tree with them.
fn accepts_pushes(upstream: &TempRepo) {
	upstream.git(&["config", "receive.denyCurrentBranch", "updateInstead"]);
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

/// Pull takes no argument; Push only its upstream's remote and HEAD:refs/heads/<branch>, which
/// remote::push reads from the branch's config. Nothing else reaches git.
#[test]
fn pull_and_push_refuse_every_other_argument() {
	let (_upstream, clone) = upstream_and_clone("no-extra");
	for (verb, extra) in [
		(Verb::Pull, &["--rebase"][..]),
		(Verb::Pull, &["origin", "other"][..]),
		(Verb::Push, &[][..]),
		(Verb::Push, &["--force"][..]),
		(Verb::Push, &["--tags"][..]),
		(Verb::Push, &["origin"][..]),
		(Verb::Push, &["--mirror", "HEAD:refs/heads/main"][..]),
		(Verb::Push, &["origin", "+HEAD:refs/heads/main"][..]),
		(Verb::Push, &["origin", "refs/heads/*:refs/heads/*"][..]),
		(Verb::Push, &["origin", "HEAD:refs/tags/v1"][..]),
		(
			Verb::Push,
			&["origin", "HEAD:refs/heads/main", "HEAD:refs/heads/x"][..],
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
