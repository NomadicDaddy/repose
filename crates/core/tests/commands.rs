//! Assertions A1 and A3: no remote verb exists, every spawn carries the shared environment, and a
//! commit never passes --no-verify. Plus the pipe-capacity cases Morticia found on cdfc251: output
//! larger than the pipe buffer on a read and on a hook must not deadlock.
#![allow(clippy::expect_used)]

mod support;

use std::path::Path;
use std::time::{Duration, Instant};

use repose_core::git::{Verb, commands, run};
use support::TempRepo;

#[test]
fn large_status_output_does_not_deadlock() {
	let repo = TempRepo::new("big-status");
	// About 130 KB of porcelain output, past any pipe buffer (64 KB on Windows).
	for i in 0..5000 {
		repo.write(&format!("files/untracked-{i:04}.txt"), "x\n");
	}
	let started = Instant::now();
	let out = run(
		&repo.path,
		Verb::Status,
		&["--porcelain=v2", "-z", "--untracked-files=all"],
	)
	.expect("status over the pipe capacity");
	assert!(out.len() > 65_536, "got {} bytes", out.len());
	assert!(
		started.elapsed() < Duration::from_secs(5),
		"took {:?}",
		started.elapsed()
	);
}

#[test]
fn chatty_hook_on_a_write_does_not_deadlock() {
	let repo = TempRepo::new("chatty-hook");
	repo.write("a.txt", "a\n");
	repo.git(&["add", "a.txt"]);
	repo.write(
		".githooks/pre-commit",
		"#!/bin/sh\nhead -c 300000 /dev/zero | tr '\\0' 'a'\necho\nexit 0\n",
	);
	repo.git(&["config", "core.hooksPath", ".githooks"]);
	let started = Instant::now();
	let result = run(&repo.path, Verb::Commit, &["-q", "-m", "chatty"]);
	assert!(result.is_ok(), "commit failed: {result:?}");
	assert!(
		started.elapsed() < Duration::from_secs(20),
		"took {:?}",
		started.elapsed()
	);
	let log = repo.git(&["log", "--oneline"]);
	assert!(log.contains("chatty"));
}

#[test]
fn no_remote_commands() {
	for verb in Verb::ALL {
		for word in verb.args() {
			assert!(
				!["fetch", "pull", "push", "clone", "ls-remote"].contains(word),
				"{word} is a network verb"
			);
		}
	}
	assert_eq!(Verb::ALL.len(), 12);
}

#[test]
fn every_spawn_sets_literal_pathspecs_and_reads_set_optional_locks_off() {
	let read = commands::command(Path::new("."), Verb::Status, &["--porcelain=v2"]);
	let env: Vec<(String, String)> = read
		.get_envs()
		.filter_map(|(k, v)| {
			Some((
				k.to_string_lossy().to_string(),
				v?.to_string_lossy().to_string(),
			))
		})
		.collect();
	assert!(env.contains(&("GIT_LITERAL_PATHSPECS".into(), "1".into())));
	assert!(env.contains(&("GIT_OPTIONAL_LOCKS".into(), "0".into())));
	assert!(env.contains(&("GIT_TERMINAL_PROMPT".into(), "0".into())));
	let args: Vec<String> = read
		.get_args()
		.map(|a| a.to_string_lossy().to_string())
		.collect();
	assert!(args.contains(&"core.fsmonitor=false".to_string()));

	let write = commands::command(Path::new("."), Verb::Commit, &["-F", "msg.txt"]);
	let wenv: Vec<String> = write
		.get_envs()
		.map(|(k, _)| k.to_string_lossy().to_string())
		.collect();
	assert!(wenv.contains(&"GIT_LITERAL_PATHSPECS".to_string()));
	assert!(
		!wenv.contains(&"GIT_OPTIONAL_LOCKS".to_string()),
		"writes keep optional locks"
	);
}

// CREATE_NO_WINDOW on child processes cannot be read back from a Command, so it has no unit
// test; the release build was run against the 77 repositories under the default roots on
// 2026-11-01 with the console host count unchanged before and after, which is the check that
// matters.

#[test]
fn no_verify_is_never_passed() {
	let write = commands::command(Path::new("."), Verb::Commit, &["-F", "msg.txt"]);
	let args: Vec<String> = write
		.get_args()
		.map(|a| a.to_string_lossy().to_string())
		.collect();
	assert!(!args.iter().any(|a| a == "--no-verify" || a == "--amend"));
	assert!(Verb::Commit.is_write() && !Verb::Status.is_write());
}
