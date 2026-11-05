//! Writes take their own permits (Phillip #33648 on the untimed-writes audit): `POOL_SIZE` commits
//! whose pre-commit hooks are still running leave every read permit free, so a status read in
//! another repository answers at once instead of waiting for a hook. Its own test binary, so no
//! other test shares the process-wide pools.
#![allow(clippy::expect_used)]

mod support;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use repose_core::git::pool::POOL_SIZE;
use repose_core::git::read_status;
use repose_core::git::write::commit;
use support::TempRepo;

/// A repository with one staged file and a pre-commit hook that writes `ready` once it runs, so
/// its commit holds a write permit from then on, and waits until `go` exists.
fn blocked_commit_repo(label: &str, ready: &Path, go: &Path) -> TempRepo {
	let repo = TempRepo::new(label);
	repo.write("a.txt", "a\n");
	repo.commit_all("base");
	repo.write("a.txt", "a2\n");
	repo.git(&["add", "a.txt"]);
	let hooks = repo.path.join(".githooks");
	std::fs::create_dir_all(&hooks).expect("hooks dir");
	let sh = |p: &Path| p.to_string_lossy().replace('\\', "/");
	let (ready, go) = (sh(ready), sh(go));
	let script = format!(
		"#!/bin/sh\n: > \"{ready}\"\nwhile [ ! -f \"{go}\" ]; do sleep 0.2; done\nexit 0\n"
	);
	std::fs::write(hooks.join("pre-commit"), script).expect("hook");
	repo.git(&["config", "core.hooksPath", &hooks.to_string_lossy()]);
	repo
}

#[test]
fn reads_answer_while_every_write_permit_is_held_by_a_hook() {
	let markers = TempRepo::new("pool-markers");
	let mut repos = Vec::new();
	let mut workers = Vec::new();
	let mut release: Vec<PathBuf> = Vec::new();
	let mut ready: Vec<PathBuf> = Vec::new();
	for n in 0..POOL_SIZE {
		let go = markers.path.join(format!("go-{n}"));
		let up = markers.path.join(format!("ready-{n}"));
		let repo = blocked_commit_repo(&format!("pool-w{n}"), &up, &go);
		let dir = repo.path.clone();
		workers.push(std::thread::spawn(move || commit(&dir, "held by its hook")));
		repos.push(repo);
		release.push(go);
		ready.push(up);
	}
	// Every hook is running, so every commit holds its permit, before the read starts (Roger and
	// Morticia on 5556e97: a fixed sleep could let the read in early and pass falsely).
	let deadline = Instant::now() + Duration::from_secs(60);
	let mut started = false;
	while Instant::now() < deadline {
		if ready.iter().all(|p| p.exists()) {
			started = true;
			break;
		}
		std::thread::sleep(Duration::from_millis(100));
	}
	let reader = TempRepo::new("pool-reader");
	reader.write("r.txt", "r\n");
	let answered = started.then(|| {
		let (tx, rx) = mpsc::channel();
		let dir = reader.path.clone();
		std::thread::spawn(move || {
			let _ = tx.send(read_status(&dir).is_ok());
		});
		rx.recv_timeout(Duration::from_secs(5))
	});
	// Every hook is released and every commit joined before any assert, so a failure leaves no
	// hook looping on its go marker after the test (Morticia L-1 on 4d69b2c).
	for marker in &release {
		std::fs::write(marker, "go").expect("marker");
	}
	for w in workers {
		w.join()
			.expect("commit thread")
			.expect("the held commit lands once released");
	}
	assert!(started, "not every hook started within 60 s");
	assert_eq!(
		answered,
		Some(Ok(true)),
		"a status read waited behind {POOL_SIZE} commits held by their hooks"
	);
	drop(repos);
}
