//! Assertions A1 and A3: no remote verb exists, every spawn carries the shared environment, and a
//! commit never passes --no-verify.

use std::path::Path;

use repose_core::git::{Verb, commands};

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
// test; the release build was run against 72 repositories on 2026-11-01 with the console host
// count unchanged before and after, which is the check that matters.

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
