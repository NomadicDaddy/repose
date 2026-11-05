//! Assertions A1 and A3: fetch is the only remote verb, and only as the user's Fetch; every spawn
//! carries the shared environment, and a
//! commit never passes --no-verify. Plus the pipe-capacity cases Morticia found on cdfc251: output
//! larger than the pipe buffer on a read and on a hook must not deadlock, and a spawn reads EOF
//! from its null stdin.
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
fn fetch_is_the_only_remote_verb() {
	for verb in Verb::ALL {
		for word in verb.args() {
			assert!(
				!["pull", "push", "clone", "ls-remote", "update"].contains(word),
				"{word} reaches a remote"
			);
			if *word == "fetch" {
				assert_eq!(verb, Verb::Fetch, "only Verb::Fetch may run git fetch");
			}
		}
	}
	assert_eq!(
		Verb::Fetch.args(),
		[
			"fetch",
			"--quiet",
			"--no-recurse-submodules",
			"--no-auto-maintenance"
		]
	);
	assert_eq!(Verb::ALL.len(), 16);
}

/// Fetch never prompts: git's terminal prompt is off for every verb, and for fetch Git
/// Credential Manager is made non-interactive too, by environment and by config (Steward #35492).
#[test]
fn fetch_cannot_prompt_for_a_sign_in() {
	let cmd = commands::command(Path::new("."), Verb::Fetch, &[]);
	let env = |name: &str, want: &str| {
		cmd.get_envs()
			.any(|(k, v)| k == name && v.is_some_and(|v| v == want))
	};
	assert!(env("GIT_TERMINAL_PROMPT", "0"));
	assert!(env("GCM_INTERACTIVE", "never"));
	let args: Vec<String> = cmd
		.get_args()
		.map(|a| a.to_string_lossy().to_string())
		.collect();
	let at = args
		.iter()
		.position(|a| a == "credential.interactive=false")
		.expect("credential.interactive=false is set");
	assert_eq!(args[at - 1], "-c");
	let verb_at = args.iter().position(|a| a == "fetch").expect("fetch");
	assert!(at < verb_at, "the setting comes before the verb: {args:?}");
	// No program from a repository's config or the environment can be started to ask for one.
	for setting in ["protocol.ext.allow=never", "core.askPass="] {
		let i = args
			.iter()
			.position(|a| a == setting)
			.unwrap_or_else(|| panic!("{setting} is set: {args:?}"));
		assert!(
			i < verb_at && args[i - 1] == "-c",
			"{setting} before the verb"
		);
	}
	for removed in ["GIT_ASKPASS", "SSH_ASKPASS"] {
		assert!(
			cmd.get_envs().any(|(k, v)| k == removed && v.is_none()),
			"{removed} is removed from the fetch's environment"
		);
	}
	assert!(
		commands::run(
			&std::env::temp_dir(),
			Verb::Fetch,
			&["https://example.invalid/x.git"]
		)
		.is_err(),
		"fetch takes no URL or refspec from a caller"
	);
}

#[test]
fn every_spawn_disables_lazy_fetch() {
	for verb in Verb::ALL {
		let cmd = commands::command(Path::new("."), verb, &[]);
		let set = cmd
			.get_envs()
			.any(|(k, v)| k == "GIT_NO_LAZY_FETCH" && v.is_some_and(|v| v == "1"));
		assert!(set, "{verb:?} would let git fetch a missing object (A1)");
	}
}

#[test]
fn remote_list_rejects_extra_arguments() {
	let dir = std::env::temp_dir();
	for extra in [&["update"][..], &["show", "origin"][..]] {
		let result = commands::run(&dir, Verb::RemoteList, extra);
		assert!(result.is_err(), "remote {extra:?} must never run");
	}
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
	let wargs: Vec<String> = write
		.get_args()
		.map(|a| a.to_string_lossy().to_string())
		.collect();
	assert!(wargs.contains(&"core.fsmonitor=false".to_string()));
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

/// Every spawn gets a null stdin, so git never waits on input the app cannot give it.
/// `cat-file --batch` reads object names until EOF: with a null stdin it ends at once with no
/// output, while an open stdin would hold it until the read timeout. The watchdog keeps a
/// regression from hanging the suite.
#[test]
fn a_spawned_git_reads_eof_from_a_null_stdin() {
	let repo = TempRepo::new("null-stdin");
	let path = repo.path.clone();
	let (tx, rx) = std::sync::mpsc::channel();
	std::thread::spawn(move || {
		let started = Instant::now();
		let out = run(&path, Verb::CatFile, &["--batch"]);
		let _ = tx.send((out, started.elapsed()));
	});
	let (out, elapsed) = rx
		.recv_timeout(Duration::from_secs(60))
		.expect("cat-file --batch returned within 60 s");
	assert_eq!(out.expect("EOF ends the batch cleanly"), Vec::<u8>::new());
	assert!(
		elapsed < Duration::from_secs(5),
		"EOF at once, not after a wait: {elapsed:?}"
	);
}
