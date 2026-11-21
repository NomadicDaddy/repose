//! ai-commit-message-suggestions: the Claude Code call A1 pins, the request it is given, and what
//! its answers mean (the request itself is in suggest_request.rs). No test runs the real Claude
//! Code, so the gate never sends anything; a .cmd stand-in passed by path records what it was
//! given.
#![allow(clippy::expect_used)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use repose_core::git::Cancel;
use repose_core::suggest::{
	ARGS, CANCELLED, INSTRUCTION, NO_CLI, NOTHING_STAGED, Staged, answer, command, suggest_with,
};

/// One staged file whose diff body is `size` bytes of added lines.
fn one(path: &str, size: usize) -> Staged {
	let mut text = format!(
		"diff --git a/{path} b/{path}\nindex 1111111..2222222 100644\n--- a/{path}\n+++ b/{path}\n@@ -0,0 +1 @@\n"
	);
	while text.len() < size {
		text.push_str("+a staged line\n");
	}
	Staged {
		files: vec![(path.to_string(), 1)],
		diff: text.into_bytes(),
		..Default::default()
	}
}

#[test]
fn the_claude_call_has_no_tools_no_mcp_no_hooks_and_no_saved_session() {
	assert_eq!(
		ARGS,
		[
			"--print",
			"--output-format",
			"text",
			"--tools",
			"",
			"--safe-mode",
			"--strict-mcp-config",
			"--disable-slash-commands",
			"--no-session-persistence",
		]
	);
	let dir = Path::new("C:/empty");
	let cmd = command(OsStr::new("claude"), dir);
	assert_eq!(cmd.get_program(), "claude");
	let args: Vec<&OsStr> = cmd.get_args().collect();
	assert_eq!(args, ARGS.map(OsStr::new));
	assert_eq!(cmd.get_current_dir(), Some(dir));
	assert_eq!(
		cmd.get_envs().count(),
		0,
		"no setting is passed in the environment"
	);
}

#[test]
fn answers_map_to_a_message_or_the_reason() {
	assert_eq!(
		answer(Some(0), b"\n  Add the suggest row\n\nBody line.\n", b""),
		Ok("Add the suggest row\n\nBody line.".to_string())
	);
	assert_eq!(
		answer(Some(0), b"```text\nFix the cap\n```\n", b""),
		Ok("Fix the cap".to_string())
	);
	assert_eq!(
		answer(Some(0), b"  \n", b""),
		Err("Claude Code answered with an empty message".to_string())
	);
	assert_eq!(
		answer(
			Some(1),
			b"",
			b"first\nInvalid API key \xc2\xb7 Please run /login\n\n"
		),
		Err("Claude Code: Invalid API key · Please run /login".to_string())
	);
	assert_eq!(
		answer(Some(2), b"", b""),
		Err("Claude Code failed with exit code 2 and no message".to_string())
	);
	assert_eq!(
		answer(None, b"", b""),
		Err("Claude Code stopped with no message".to_string())
	);
}

/// A folder of .cmd stand-ins for one test, removed after it.
struct StandIns(PathBuf);

impl StandIns {
	fn new(name: &str) -> Self {
		let dir = std::env::temp_dir().join(format!("repose-test-{name}-{}", std::process::id()));
		std::fs::create_dir_all(&dir).expect("temp dir");
		Self(dir)
	}

	fn script(&self, name: &str, body: &str) -> PathBuf {
		let path = self.0.join(name);
		std::fs::write(&path, body.replace('\n', "\r\n")).expect("stand-in");
		path
	}

	fn read(&self, name: &str) -> String {
		std::fs::read_to_string(self.0.join(name)).expect("the stand-in ran")
	}
}

impl Drop for StandIns {
	fn drop(&mut self) {
		let _ = std::fs::remove_dir_all(&self.0);
	}
}

/// The stand-in records its arguments, its working folder, what that folder holds and the request
/// it read on stdin, then answers. The folder must be empty and gone afterwards.
#[test]
fn a_stand_in_gets_the_exact_call_in_an_empty_folder_and_its_answer_comes_back() {
	let dir = StandIns::new("claude");
	let ok = dir.script(
		"claude-ok.cmd",
		"@echo off\n\
		 >\"%~dp0args.txt\" echo %*\n\
		 >\"%~dp0cwd.txt\" cd\n\
		 >\"%~dp0listing.txt\" dir /b /a\n\
		 more >\"%~dp0stdin.txt\"\n\
		 echo Add the suggest row\n",
	);
	let diff = one("src/a.rs", 300);
	let got = suggest_with(ok.as_os_str(), &diff, &Cancel::new()).expect("an answer");
	assert_eq!(got.message, "Add the suggest row");
	assert!(got.left_out.is_empty());
	assert_eq!(
		dir.read("args.txt").trim(),
		"--print --output-format text --tools \"\" --safe-mode --strict-mcp-config --disable-slash-commands --no-session-persistence"
	);
	let cwd = PathBuf::from(dir.read("cwd.txt").trim());
	assert!(
		cwd.starts_with(std::env::temp_dir())
			&& cwd
				.file_name()
				.is_some_and(|n| n.to_string_lossy().starts_with("repose-suggest-")),
		"it ran in its own temporary folder, not the repository: {}",
		cwd.display()
	);
	assert_eq!(dir.read("listing.txt").trim(), "", "the folder was empty");
	assert!(!cwd.exists(), "the folder is removed afterwards");
	let sent = dir.read("stdin.txt").replace("\r\n", "\n");
	assert!(sent.starts_with(INSTRUCTION));
	assert!(
		sent.contains("+++ b/src/a.rs"),
		"the staged diff was on stdin"
	);
	let failing = dir.script(
		"claude-signed-out.cmd",
		"@echo off\nmore >nul\n>&2 echo Not logged in. Please run /login\nexit /b 1\n",
	);
	assert_eq!(
		suggest_with(failing.as_os_str(), &diff, &Cancel::new()),
		Err("Claude Code: Not logged in. Please run /login".to_string())
	);
}

#[test]
fn nothing_staged_or_cancelled_first_runs_nothing_and_a_missing_cli_says_so() {
	let dir = StandIns::new("claude-none");
	let never = dir.script(
		"claude-never.cmd",
		"@echo off\n>\"%~dp0ran.txt\" echo ran\n",
	);
	let empty = Staged::default();
	assert_eq!(
		suggest_with(never.as_os_str(), &empty, &Cancel::new()),
		Err(NOTHING_STAGED.to_string())
	);
	// A Cancel set while the staged changes were read sends nothing (Roger 5Q-1 on d3da4dd).
	let cancelled = Cancel::new();
	cancelled.cancel();
	assert_eq!(
		suggest_with(never.as_os_str(), &one("a", 100), &cancelled),
		Err(CANCELLED.to_string())
	);
	assert!(
		!dir.0.join("ran.txt").exists(),
		"nothing ran for an empty list or after Cancel"
	);
	let absent = dir.0.join("no-such-claude.exe");
	assert_eq!(
		suggest_with(absent.as_os_str(), &one("a", 100), &Cancel::new()),
		Err(NO_CLI.to_string())
	);
}

#[test]
fn cancel_stops_the_cli_and_says_so() {
	let dir = StandIns::new("claude-slow");
	let slow = dir.script(
		"claude-slow.cmd",
		"@echo off\nping -n 30 127.0.0.1 >nul\necho late\n",
	);
	let cancel = Cancel::new();
	let later = cancel.clone();
	std::thread::spawn(move || {
		std::thread::sleep(Duration::from_millis(300));
		later.cancel();
	});
	let started = Instant::now();
	assert_eq!(
		suggest_with(slow.as_os_str(), &one("a", 100), &cancel),
		Err(CANCELLED.to_string())
	);
	assert!(
		started.elapsed() < Duration::from_secs(10),
		"Cancel did not wait for the CLI"
	);
}
