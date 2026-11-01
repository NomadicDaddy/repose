//! stage-and-commit clause 9: Cancel during a commit kills git and the hook's whole process
//! tree, the commit does not happen, and an index lock left behind sits where
//! `rev-parse --git-path index.lock` says, in a main repository and in a linked worktree.
#![allow(clippy::expect_used)]

mod support;

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use repose_core::git::write::{Committed, commit_cancellable, index_lock_path};
use repose_core::git::{Cancel, GitError};
use support::TempRepo;

/// Trailing spaces and a run of blank lines, which git tidies away when it stores the message;
/// the landed check must still match it (Morticia L-4 on 4f22cd3).
const MESSAGE: &str = "never lands  \n\n\n\nwith a body\n";

/// Processes whose command line carries `marker`, by id; the pwsh asking is excluded.
fn processes_with(marker: &str) -> Vec<u32> {
	let list = format!(
		"(Get-CimInstance Win32_Process | Where-Object {{ $_.CommandLine -like '*{marker}*' -and $_.ProcessId -ne $PID }} | ForEach-Object ProcessId) -join ' '"
	);
	let out = Command::new("pwsh")
		.args(["-NoProfile", "-NonInteractive", "-Command", &list])
		.output()
		.expect("pwsh lists processes");
	String::from_utf8_lossy(&out.stdout)
		.split_whitespace()
		.filter_map(|s| s.parse().ok())
		.collect()
}

/// A pre-commit hook that starts a child sleeping 60 s, tagged with `marker`, and waits for it.
fn install_slow_hook(repo_dir: &Path, hooks_dir: &Path, hook: &str, marker: &str) {
	std::fs::create_dir_all(hooks_dir).expect("hooks dir");
	let script = format!(
		"#!/bin/sh\npwsh -NoProfile -NonInteractive -Command \"$null = '{marker}'; Start-Sleep 60\"\nexit 0\n"
	);
	std::fs::write(hooks_dir.join(hook), script).expect("hook");
	let hooks = hooks_dir.to_string_lossy().to_string();
	support::git_in(repo_dir, &["config", "core.hooksPath", &hooks]);
}

/// Starts the commit on a thread, cancels it once the hook's child is running, and returns the
/// result and how long the cancel took to come back.
fn cancelled_commit(repo_dir: &Path, marker: &str) -> (Result<Committed, GitError>, Duration) {
	let cancel = Cancel::new();
	let dir = repo_dir.to_path_buf();
	let worker = {
		let cancel = cancel.clone();
		std::thread::spawn(move || commit_cancellable(&dir, MESSAGE, Some(&cancel)))
	};
	let started = Instant::now();
	while processes_with(marker).is_empty() {
		assert!(
			started.elapsed() < Duration::from_secs(20),
			"the hook's child never appeared"
		);
		std::thread::sleep(Duration::from_millis(250));
	}
	let asked = Instant::now();
	cancel.cancel();
	let result = worker.join().expect("worker thread");
	(result, asked.elapsed())
}

fn assert_cancelled(repo_dir: &Path, marker: &str, lock_under: &Path) {
	let (result, took) = cancelled_commit(repo_dir, marker);
	assert!(
		matches!(
			result,
			Err(GitError::Cancelled {
				tree_killed: true,
				..
			})
		),
		"{result:?}"
	);
	assert!(took < Duration::from_secs(10), "cancel took {took:?}");
	std::thread::sleep(Duration::from_millis(500));
	let left = processes_with(marker);
	for pid in &left {
		let _ = Command::new("taskkill")
			.args(["/PID", &pid.to_string(), "/F"])
			.output();
	}
	assert!(
		left.is_empty(),
		"the hook's child survived Cancel: {left:?}"
	);
	assert!(
		!support::git_in(repo_dir, &["log", "--oneline", "--all"]).contains("never lands"),
		"the commit must not land"
	);
	let lock = index_lock_path(repo_dir).expect("lock path");
	assert!(
		lock.starts_with(lock_under),
		"lock {} is not under {}",
		lock.display(),
		lock_under.display()
	);
	// Whether index.lock is left depends on where git was when it was killed: one run of this
	// test left it, the next did not, so its presence is not asserted. When it is there, the app
	// reports it (leftover_lock) and never removes it; the test removes it so the temp folder can
	// be dropped.
	if lock.exists() {
		std::fs::remove_file(&lock).expect("remove the test's lock");
	}
}

#[test]
fn cancel_kills_the_hook_tree_in_a_repository_and_in_a_worktree() {
	let root = TempRepo::new("cancel");
	let main = root.path.join("main");
	std::fs::create_dir_all(&main).expect("main dir");
	support::git_in(&main, &["init", "-q", "-b", "main"]);
	support::git_in(&main, &["config", "core.autocrlf", "false"]);
	std::fs::write(main.join("a.txt"), "a\n").expect("write");
	support::git_in(&main, &["add", "a.txt"]);
	support::git_in(&main, &["commit", "-q", "-m", "base"]);
	let hooks = main.join(".githooks");
	let marker_main = format!("repose-cancel-main-{}", std::process::id());
	install_slow_hook(&main, &hooks, "pre-commit", &marker_main);
	std::fs::write(main.join("a.txt"), "a2\n").expect("write");
	support::git_in(&main, &["add", "a.txt"]);
	assert_cancelled(&main, &marker_main, &main.join(".git"));

	let wt = root.path.join("task");
	let wt_arg = wt.to_string_lossy().to_string();
	support::git_in(&main, &["worktree", "add", "-q", &wt_arg, "-b", "task"]);
	let marker_wt = format!("repose-cancel-wt-{}", std::process::id());
	install_slow_hook(&wt, &hooks, "pre-commit", &marker_wt);
	std::fs::write(wt.join("b.txt"), "b\n").expect("write");
	support::git_in(&wt, &["add", "b.txt"]);
	assert_cancelled(&wt, &marker_wt, &main.join(".git").join("worktrees"));

	// Roger #31030 on a31e63b: a Cancel while a post-commit hook runs comes after HEAD moved, so
	// the answer is Ok with hook_stopped, never "cancelled", and the commit is there.
	std::fs::remove_file(hooks.join("pre-commit")).expect("drop the pre-commit hook");
	let marker_post = format!("repose-cancel-post-{}", std::process::id());
	install_slow_hook(&main, &hooks, "post-commit", &marker_post);
	std::fs::write(main.join("a.txt"), "a3\n").expect("write");
	support::git_in(&main, &["add", "a.txt"]);
	let (result, _) = cancelled_commit(&main, &marker_post);
	std::thread::sleep(Duration::from_millis(500));
	for pid in processes_with(&marker_post) {
		let _ = Command::new("taskkill")
			.args(["/PID", &pid.to_string(), "/F"])
			.output();
	}
	assert!(
		matches!(
			result,
			Ok(Committed {
				hook_stopped: true,
				tree_killed: true
			})
		),
		"{result:?}"
	);
	assert!(
		support::git_in(&main, &["log", "--oneline"]).contains("never lands"),
		"the commit landed before the cancel"
	);
}
