//! Assertions A2 and A3 for the write verbs: literal pathspecs (Morticia's [1].txt pair), hooks
//! that run, fail, print a lot, read stdin or take longer than the read timeout, and the lock path.
#![allow(clippy::expect_used)]

mod support;

use std::time::{Duration, Instant};

use repose_core::git::status::read_status;
use repose_core::git::write::{commit, discard, index_lock_path, stage, unstage, unstage_all};
use repose_core::git::{GitError, Verb, run};
use repose_core::model::{ChangeKind, Changes, DiscardPlan};
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

/// Morticia M-1 on 4cc9651: `git add -N` leaves an empty index entry, so a restore from the index
/// empties the working file. The plan the view follows must refuse it, and the content must stay.
#[test]
fn intent_to_add_is_refused_by_the_discard_plan() {
	let repo = TempRepo::new("intent-to-add");
	repo.write("a.txt", "a\n");
	repo.commit_all("base");
	repo.write("new.txt", "precious\n");
	repo.git(&["add", "-N", "new.txt"]);
	let porcelain = read_status(&repo.path).expect("status");
	let changes = Changes::from_entries(&porcelain.entries);
	let entry = changes
		.unstaged
		.iter()
		.find(|f| f.path == "new.txt")
		.expect("listed in the Changes group");
	assert_eq!(entry.kind, ChangeKind::IntentToAdd);
	assert_eq!(entry.kind.letter(), "A");
	assert!(
		matches!(entry.discard_plan(), DiscardPlan::Refused(_)),
		"{:?}",
		entry.discard_plan()
	);
	let text = std::fs::read_to_string(repo.path.join("new.txt")).expect("read");
	assert_eq!(text, "precious\n");
}

/// Morticia L-3: `git clean -f` exits 0 and leaves a nested repository; that is not a success.
#[test]
fn discard_reports_a_nested_repository_it_could_not_remove() {
	let repo = TempRepo::new("nested");
	repo.write("a.txt", "a\n");
	repo.commit_all("base");
	repo.write("inner/file.txt", "inner\n");
	support::git_in(&repo.path.join("inner"), &["init", "-q"]);
	let err = discard(&repo.path, &[], &["inner/".to_string()]).expect_err("still present");
	assert!(
		matches!(err, GitError::StillPresent(ref p) if p == &["inner/".to_string()]),
		"{err}"
	);
	assert!(
		repo.path.join("inner/file.txt").exists(),
		"nothing inside it was touched"
	);
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
fn unstage_works_before_the_first_commit() {
	let repo = TempRepo::new("unborn-unstage");
	repo.write("a.txt", "a\n");
	repo.write("b.txt", "b\n");
	stage(&repo.path, &["a.txt".to_string(), "b.txt".to_string()]).expect("stage");
	assert!(status(&repo).contains("A  a.txt"), "{}", status(&repo));
	unstage(&repo.path, &["a.txt".to_string()]).expect("unstage on an unborn branch");
	let s = status(&repo);
	assert!(s.contains("?? a.txt"), "a.txt back to untracked: {s}");
	assert!(s.contains("A  b.txt"), "b.txt still staged: {s}");
	assert!(repo.path.join("a.txt").exists(), "the working file stays");
}

/// Morticia M-1 and Roger on 51bc93d: a HEAD check that fails for any other reason must not be
/// read as "no commits yet", or Unstage all stages the deletion of a committed tree.
#[test]
fn unstage_all_refuses_to_guess_when_the_head_check_fails() {
	let repo = TempRepo::new("head-check");
	repo.write("a.txt", "a\n");
	repo.commit_all("base");
	repo.write("a.txt", "a2\n");
	repo.git(&["add", "a.txt"]);
	let head_file = repo.path.join(".git/HEAD");
	let saved = std::fs::read(&head_file).expect("HEAD");
	// An unreadable HEAD makes git refuse the repository: exit 128, not the unborn exit 1.
	std::fs::write(&head_file, b"not a ref\n").expect("corrupt HEAD");
	let err = unstage_all(&repo.path).expect_err("the check failed, so nothing is chosen");
	std::fs::write(&head_file, saved).expect("restore HEAD");
	assert!(
		!matches!(err, GitError::Failed { code: Some(1), .. }),
		"a real failure, not the unborn answer: {err}"
	);
	let s = status(&repo);
	assert!(s.starts_with("M  a.txt"), "index untouched: {s}");
	assert!(!s.contains("D  a.txt"), "nothing staged for deletion: {s}");
}

/// Morticia L-5 on cba0c7a: a corrupt branch ref gives the same exit 1 as an unborn branch, and
/// must not be treated as one.
#[test]
fn unstage_all_refuses_a_corrupt_branch_ref() {
	let repo = TempRepo::new("corrupt-ref");
	repo.write("a.txt", "a\n");
	repo.commit_all("base");
	repo.write("a.txt", "a2\n");
	repo.git(&["add", "a.txt"]);
	let ref_file = repo.path.join(".git/refs/heads/main");
	let saved = std::fs::read(&ref_file).expect("ref file (loose, not packed, in a fresh repo)");
	std::fs::write(&ref_file, b"garbage\n").expect("corrupt ref");
	let err = unstage_all(&repo.path).expect_err("a broken ref is not an unborn branch");
	std::fs::write(&ref_file, saved).expect("restore ref");
	// git itself answers differently by version: symbolic-ref may refuse HEAD outright, or name
	// the branch whose file is then found to exist. Either way it is UnreadableRef, never Unborn.
	assert!(matches!(err, GitError::UnreadableRef(_)), "{err}");
	let s = status(&repo);
	assert!(s.starts_with("M  a.txt"), "index untouched: {s}");
}

/// Morticia L-1 on 51bc93d: a staged file edited again (AM) blocked `rm --cached` without --force.
#[test]
fn unstage_before_the_first_commit_handles_an_edited_staged_file() {
	let repo = TempRepo::new("unborn-am");
	repo.write("b.txt", "b\n");
	stage(&repo.path, &["b.txt".to_string()]).expect("stage");
	repo.write("b.txt", "b edited\n");
	assert!(status(&repo).contains("AM b.txt"), "{}", status(&repo));
	unstage_all(&repo.path).expect("unstage all with an AM file");
	assert!(status(&repo).contains("?? b.txt"), "{}", status(&repo));
	let text = std::fs::read_to_string(repo.path.join("b.txt")).expect("read");
	assert_eq!(text, "b edited\n", "the working file keeps the edit");
}

#[test]
fn failing_hook_blocks_the_commit_and_keeps_the_index() {
	let repo = TempRepo::new("hook-fail");
	repo.write("a.txt", "a\n");
	repo.commit_all("base");
	repo.write("a.txt", "a2\n");
	repo.git(&["add", "a.txt"]);
	// Ten lines, so the user sees the hook's whole complaint and not a three-line tail.
	repo.write(
		".githooks/pre-commit",
		"#!/bin/sh\nfor i in 1 2 3 4 5 6 7 8 9 10; do echo \"hook line $i\" >&2; done\necho 'hook says no' >&2\nexit 1\n",
	);
	repo.git(&["config", "core.hooksPath", ".githooks"]);
	let err = commit(&repo.path, "blocked").expect_err("hook blocks");
	let text = err.to_string();
	assert!(text.contains("hook says no"), "{err}");
	assert!(
		text.contains("hook line 1\n") && text.contains("hook line 10\n"),
		"the whole output, line by line: {err}"
	);
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

/// A hook that reads stdin sees EOF at once and its refusal reaches the user with the index kept.
/// Git (2.56) gives a pre-commit hook a null stdin itself, so this pins the outcome; that repose's
/// own spawn has a null stdin is pinned in tests/commands.rs. The commit runs behind a watchdog,
/// so a hook that blocked would fail this test instead of hanging the suite.
#[test]
fn hook_reading_stdin_sees_eof_and_blocks_the_commit() {
	let repo = TempRepo::new("hook-stdin");
	repo.write("a.txt", "a\n");
	repo.commit_all("base");
	repo.write("a.txt", "a2\n");
	repo.git(&["add", "a.txt"]);
	repo.write(
		".githooks/pre-commit",
		"#!/bin/sh\nif read line; then echo \"hook read a line: $line\" >&2; exit 1; fi\necho 'hook saw EOF on stdin' >&2\nexit 1\n",
	);
	repo.git(&["config", "core.hooksPath", ".githooks"]);
	let path = repo.path.clone();
	let (tx, rx) = std::sync::mpsc::channel();
	std::thread::spawn(move || {
		let _ = tx.send(commit(&path, "reads stdin"));
	});
	let outcome = rx
		.recv_timeout(Duration::from_secs(60))
		.expect("the hook got EOF within 60 s; an open stdin would block it here");
	let err = outcome.expect_err("the hook blocks the commit");
	assert!(err.to_string().contains("hook saw EOF on stdin"), "{err}");
	assert!(status(&repo).starts_with("M  a.txt"), "index unchanged");
	assert!(!repo.git(&["log", "--oneline"]).contains("reads stdin"));
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
