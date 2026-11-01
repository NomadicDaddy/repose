//! Assertions A2 and A3 for the write verbs: literal pathspecs (Morticia's [1].txt pair), hooks
//! that run, fail, print a lot or take longer than the read timeout, and the lock path.
#![allow(clippy::expect_used)]

mod support;

use std::time::{Duration, Instant};

use repose_core::git::write::{commit, discard, index_lock_path, stage, unstage};
use repose_core::git::{Verb, run};
use support::TempRepo;

fn status(repo: &TempRepo) -> String {
	repo.git(&["status", "--porcelain"])
}

#[test]
fn discard_touches_only_the_listed_file_even_with_glob_characters() {
	let repo = TempRepo::new("literal");
	repo.write("1.txt", "one\n");
	repo.write("[1].txt", "bracket\n");
	repo.commit_all("base");
	repo.write("1.txt", "one edited\n");
	repo.write("[1].txt", "bracket edited\n");
	discard(&repo.path, &["[1].txt".to_string()], &[]).expect("discard");
	let s = status(&repo);
	assert!(s.contains(" M 1.txt"), "1.txt keeps its edit: {s}");
	assert!(!s.contains("[1].txt"), "[1].txt restored: {s}");

	repo.write("2.txt", "two\n");
	repo.write("[2].txt", "bracket two\n");
	discard(&repo.path, &[], &["[2].txt".to_string()]).expect("clean");
	assert!(repo.path.join("2.txt").exists(), "2.txt stays");
	assert!(!repo.path.join("[2].txt").exists(), "[2].txt removed");
}

#[test]
fn stage_and_unstage_round_trip() {
	let repo = TempRepo::new("stage");
	repo.write("a.txt", "a\n");
	repo.commit_all("base");
	repo.write("a.txt", "a2\n");
	stage(&repo.path, &["a.txt".to_string()]).expect("stage");
	assert!(
		status(&repo).starts_with("M  a.txt"),
		"staged: {}",
		status(&repo)
	);
	unstage(&repo.path, &["a.txt".to_string()]).expect("unstage");
	assert!(
		status(&repo).starts_with(" M a.txt"),
		"unstaged: {}",
		status(&repo)
	);
}

#[test]
fn failing_hook_blocks_the_commit_and_keeps_the_index() {
	let repo = TempRepo::new("hook-fail");
	repo.write("a.txt", "a\n");
	repo.commit_all("base");
	repo.write("a.txt", "a2\n");
	repo.git(&["add", "a.txt"]);
	repo.write(
		".githooks/pre-commit",
		"#!/bin/sh\necho 'hook says no' >&2\nexit 1\n",
	);
	repo.git(&["config", "core.hooksPath", ".githooks"]);
	let err = commit(&repo.path, "blocked").expect_err("hook blocks");
	assert!(err.to_string().contains("hook says no"), "{err}");
	assert!(status(&repo).starts_with("M  a.txt"), "index unchanged");
	assert!(!repo.git(&["log", "--oneline"]).contains("blocked"));
}

#[test]
fn slow_hook_is_not_timed_out() {
	let repo = TempRepo::new("hook-slow");
	repo.write("a.txt", "a\n");
	repo.git(&["add", "a.txt"]);
	repo.write(".githooks/pre-commit", "#!/bin/sh\nsleep 12\nexit 0\n");
	repo.git(&["config", "core.hooksPath", ".githooks"]);
	let started = Instant::now();
	commit(&repo.path, "slow but fine").expect("commit after a 12 s hook");
	assert!(started.elapsed() >= Duration::from_secs(11));
	assert!(repo.git(&["log", "--oneline"]).contains("slow but fine"));
}

#[test]
fn commit_uses_the_repository_identity_and_never_no_verify() {
	let repo = TempRepo::new("identity");
	repo.write("a.txt", "a\n");
	repo.git(&["add", "a.txt"]);
	repo.git(&["config", "user.name", "Repose Test"]);
	repo.git(&["config", "user.email", "repose@example.invalid"]);
	commit(&repo.path, "first\n\nbody line").expect("commit");
	let author = repo.git(&["log", "-1", "--format=%an <%ae>"]);
	assert_eq!(author.trim(), "Repose Test <repose@example.invalid>");
	assert_eq!(
		repo.git(&["log", "-1", "--format=%B"]).trim(),
		"first\n\nbody line"
	);
	// The commit function builds its own arguments; the only way to pass --no-verify would be
	// through run, and Verb::Commit's args are fixed to "commit".
	assert_eq!(Verb::Commit.args(), &["commit"]);
}

#[test]
fn index_lock_path_follows_worktrees() {
	let main = TempRepo::new("lock-main");
	main.write("a.txt", "a\n");
	main.commit_all("base");
	let wt = main.path.with_file_name(format!(
		"{}-wt",
		main.path
			.file_name()
			.map(|n| n.to_string_lossy().to_string())
			.unwrap_or_default()
	));
	let wt_str = wt.to_string_lossy().to_string();
	main.git(&["worktree", "add", "-q", "-b", "feature", &wt_str]);
	let main_lock = index_lock_path(&main.path).expect("main lock path");
	assert!(main_lock.ends_with(".git/index.lock") || main_lock.ends_with(".git\\index.lock"));
	let wt_lock = index_lock_path(&wt).expect("worktree lock path");
	let text = wt_lock.to_string_lossy().replace('\\', "/");
	assert!(
		text.contains("/.git/worktrees/"),
		"worktree lock under main: {text}"
	);
	let _ = run(&main.path, Verb::RevParse, &["--git-dir"]);
	let _ = std::fs::remove_dir_all(&wt);
}
