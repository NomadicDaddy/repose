//! A throwaway repository under the OS temp folder, removed on drop. Uses the installed git with
//! a fixed identity and no hooks, so the tests never touch the developer's config.
#![allow(clippy::expect_used, dead_code)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use repose_core::git::Cancel;

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
		set_identity(&repo.path);
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

/// Gives the repository at `dir` its own committer identity. The code under test runs git with the
/// machine's global config, and a CI runner has no identity there, so a commit it makes would be
/// refused with "Author identity unknown" (CI run 42920036918).
pub fn set_identity(dir: &Path) {
	git_in(dir, &["config", "user.name", "Test"]);
	git_in(dir, &["config", "user.email", "test@example.invalid"]);
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

/// An upstream with one commit and a clone of it, so the clone's origin/main is set.
pub fn upstream_and_clone(label: &str) -> (TempRepo, TempRepo) {
	let upstream = TempRepo::new(&format!("{label}-upstream"));
	upstream.write("a.txt", "1\n");
	upstream.commit_all("one");
	let clone = TempRepo::new(&format!("{label}-clone"));
	std::fs::remove_dir_all(clone.path.join(".git")).expect("clear");
	let src = upstream.path.to_string_lossy().replace('\\', "/");
	let dst = clone.path.to_string_lossy().to_string();
	upstream.git(&["clone", "-q", &src, &dst]);
	(upstream, clone)
}

/// Sets a Cancel after `limit`, so a fetch that waited on a prompt or a window is killed and the
/// test fails instead of hanging the gate.
pub fn watchdog(limit: Duration) -> (Cancel, Arc<AtomicBool>) {
	let cancel = Cancel::new();
	let done = Arc::new(AtomicBool::new(false));
	let (c, d) = (cancel.clone(), done.clone());
	std::thread::spawn(move || {
		let started = Instant::now();
		while started.elapsed() < limit {
			if d.load(Ordering::SeqCst) {
				return;
			}
			std::thread::sleep(Duration::from_millis(100));
		}
		c.cancel();
	});
	(cancel, done)
}

/// A 127.0.0.1 listener that answers every request with 401 and a Basic challenge, which is what
/// an https host asking for a sign-in sends; returns its port. Nothing leaves the machine.
pub fn sign_in_listener() -> u16 {
	let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1");
	let port = listener.local_addr().expect("addr").port();
	std::thread::spawn(move || {
		for stream in listener.incoming() {
			let Ok(mut s) = stream else { continue };
			let mut buf = [0u8; 4096];
			let _ = s.read(&mut buf);
			let _ = s.write_all(
				b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"repose-test\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
			);
		}
	});
	port
}
