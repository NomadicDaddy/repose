//! A throwaway repository under the OS temp folder, removed on drop. Uses the installed git with
//! a fixed identity and no hooks, so the tests never touch the developer's config.
#![allow(clippy::expect_used, dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct TempRepo {
	pub path: PathBuf,
}

impl TempRepo {
	pub fn new(label: &str) -> TempRepo {
		let unique = format!(
			"repose-test-{label}-{}-{}",
			std::process::id(),
			std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.map(|d| d.as_nanos())
				.unwrap_or(0)
		);
		let path = std::env::temp_dir().join(unique);
		std::fs::create_dir_all(&path).expect("temp dir");
		let repo = TempRepo { path };
		repo.git(&["init", "-q", "-b", "main"]);
		// The code under test runs git with the developer's own global config, this harness runs
		// it with none; pin line endings locally so the two agree on what "unchanged" means.
		repo.git(&["config", "core.autocrlf", "false"]);
		repo
	}

	pub fn git(&self, args: &[&str]) -> String {
		git_in(&self.path, args)
	}

	pub fn write(&self, rel: &str, text: &str) {
		let p = self.path.join(rel);
		if let Some(dir) = p.parent() {
			std::fs::create_dir_all(dir).expect("dir");
		}
		std::fs::write(p, text).expect("write");
	}

	pub fn commit_all(&self, message: &str) {
		self.git(&["add", "-A"]);
		self.git(&["commit", "-q", "-m", message]);
	}

	/// Runs git with `input` on its standard input, as `git` runs it otherwise. One
	/// `git fast-import` stream builds a long history in one spawn rather than one per commit.
	pub fn git_with_input(&self, args: &[&str], input: &str) -> String {
		let mut child = git_command(&self.path, args)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.spawn()
			.expect("git runs");
		child
			.stdin
			.take()
			.expect("stdin")
			.write_all(input.as_bytes())
			.expect("git reads its input");
		let out = child.wait_with_output().expect("git finishes");
		assert!(out.status.success(), "git {args:?} failed");
		String::from_utf8_lossy(&out.stdout).to_string()
	}
}

pub fn git_in(dir: &Path, args: &[&str]) -> String {
	let out = git_command(dir, args).output().expect("git runs");
	String::from_utf8_lossy(&out.stdout).to_string()
}

fn git_command(dir: &Path, args: &[&str]) -> Command {
	let mut command = Command::new("git");
	command
		.current_dir(dir)
		.env("GIT_CONFIG_GLOBAL", "NUL")
		.env("GIT_CONFIG_SYSTEM", "NUL")
		.env("GIT_AUTHOR_NAME", "Test")
		.env("GIT_AUTHOR_EMAIL", "test@example.invalid")
		.env("GIT_COMMITTER_NAME", "Test")
		.env("GIT_COMMITTER_EMAIL", "test@example.invalid")
		.args(["-c", "core.hooksPath=/dev/null"])
		.args(args);
	command
}

impl Drop for TempRepo {
	fn drop(&mut self) {
		let _ = std::fs::remove_dir_all(&self.path);
	}
}
