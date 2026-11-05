//! remote-sync: Fetch, the one network verb (A1 as revised, Phillip #35530). It brings the
//! remote's commits in, reports a failure with git's own message and leaves the refs alone, and a
//! sign-in request fails instead of prompting: no terminal prompt and no Git Credential Manager
//! window (Steward #35492). The remotes are a local bare repository and a 127.0.0.1 listener, so
//! nothing leaves the machine.
#![allow(clippy::expect_used)]

mod support;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use repose_core::git::remote::fetch;
use repose_core::git::{Cancel, GitError, read_status};
use support::{TempRepo, git_in};

/// An upstream with one commit and a clone of it, so the clone's origin/main is set.
fn upstream_and_clone(label: &str) -> (TempRepo, TempRepo) {
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
fn watchdog(limit: Duration) -> (Cancel, Arc<AtomicBool>) {
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

#[test]
fn fetch_brings_the_remotes_new_commits_in() {
	let (upstream, clone) = upstream_and_clone("fetch-ok");
	upstream.write("b.txt", "2\n");
	upstream.commit_all("two");
	assert_eq!(read_status(&clone.path).expect("status").behind, Some(0));
	let (cancel, done) = watchdog(Duration::from_secs(60));
	let result = fetch(&clone.path, Some(&cancel));
	done.store(true, Ordering::SeqCst);
	assert!(result.is_ok(), "fetch failed: {result:?}");
	let after = read_status(&clone.path).expect("status");
	assert_eq!(after.upstream.as_deref(), Some("origin/main"));
	assert_eq!(after.behind, Some(1), "the new upstream commit is counted");
	assert_eq!(after.ahead, Some(0));
}

#[test]
fn a_failed_fetch_names_gits_error_and_leaves_the_refs_alone() {
	let (_upstream, clone) = upstream_and_clone("fetch-gone");
	let missing = clone.path.with_file_name("repose-test-no-such-remote");
	let missing = missing.to_string_lossy().replace('\\', "/");
	clone.git(&["remote", "set-url", "origin", &missing]);
	let before = clone.git(&["for-each-ref"]);
	let (cancel, done) = watchdog(Duration::from_secs(60));
	let result = fetch(&clone.path, Some(&cancel));
	done.store(true, Ordering::SeqCst);
	match result {
		Err(GitError::Failed { stderr_tail, .. }) => {
			assert!(
				stderr_tail.contains("repose-test-no-such-remote"),
				"git's message names the remote it could not reach: {stderr_tail:?}"
			);
		}
		other => panic!("expected git's failure, got {other:?}"),
	}
	assert_eq!(
		clone.git(&["for-each-ref"]),
		before,
		"no ref moved on failure"
	);
}

#[test]
fn a_sign_in_request_fails_without_a_prompt_or_window() {
	// Every request gets 401 with a Basic challenge: what an https host asking for a sign-in sends.
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
	let repo = TempRepo::new("fetch-401");
	repo.write("a.txt", "a\n");
	repo.commit_all("one");
	let url = format!("http://127.0.0.1:{port}/team/thing.git");
	repo.git(&["remote", "add", "origin", &url]);
	let started = Instant::now();
	let (cancel, done) = watchdog(Duration::from_secs(60));
	let result = fetch(&repo.path, Some(&cancel));
	done.store(true, Ordering::SeqCst);
	let took = started.elapsed();
	assert!(
		!matches!(result, Err(GitError::Cancelled { .. })),
		"fetch waited {took:?} for a sign-in: a prompt or a credential window held it"
	);
	match result {
		// With Git Credential Manager as the helper (Git for Windows' default), GCM itself says it
		// may not prompt; with no helper, git says its terminal prompt is off. Either is a refusal
		// to ask, never a window.
		Err(GitError::Failed { stderr_tail, .. }) => assert!(
			stderr_tail.contains("interactivity has been disabled")
				|| stderr_tail.contains("terminal prompts disabled"),
			"the failure is a refusal to prompt: {stderr_tail:?}"
		),
		other => panic!("a sign-in request must fail with git's message, got {other:?}"),
	}
	assert!(
		git_in(&repo.path, &["for-each-ref", "refs/remotes"]).is_empty(),
		"nothing was fetched"
	);
}
