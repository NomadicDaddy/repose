//! Writes take their own permits (Phillip #33648 on the untimed-writes audit): `POOL_SIZE` commits
//! whose pre-commit hooks are still running leave every read permit free, so a status read in
//! another repository answers at once instead of waiting for a hook. Its own test binary, so no
//! other test shares the process-wide pools.
#![allow(clippy::expect_used)]

mod support;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use repose_core::git::pool::POOL_SIZE;
use repose_core::git::read_status;
use repose_core::git::write::commit;
use support::TempRepo;

/// A repository with one staged file and a pre-commit hook that waits until `marker` exists.
fn blocked_commit_repo(label: &str, marker: &Path) -> TempRepo {
	let repo = TempRepo::new(label);
	repo.write("a.txt", "a\n");
	repo.commit_all("base");
	repo.write("a.txt", "a2\n");
	repo.git(&["add", "a.txt"]);
	let hooks = repo.path.join(".githooks");
	std::fs::create_dir_all(&hooks).expect("hooks dir");
	let wait = marker.to_string_lossy().replace('\\', "/");
	let script = format!("#!/bin/sh\nwhile [ ! -f \"{wait}\" ]; do sleep 0.2; done\nexit 0\n");
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
	for n in 0..POOL_SIZE {
		let marker = markers.path.join(format!("go-{n}"));
		let repo = blocked_commit_repo(&format!("pool-w{n}"), &marker);
		let dir = repo.path.clone();
		workers.push(std::thread::spawn(move || commit(&dir, "held by its hook")));
		repos.push(repo);
		release.push(marker);
	}
	// Give every commit time to start git and reach its hook.
	std::thread::sleep(Duration::from_secs(3));
	let reader = TempRepo::new("pool-reader");
	reader.write("r.txt", "r\n");
	let (tx, rx) = mpsc::channel();
	let dir = reader.path.clone();
	std::thread::spawn(move || {
		let _ = tx.send(read_status(&dir).is_ok());
	});
	let answered = rx.recv_timeout(Duration::from_secs(5));
	for marker in &release {
		std::fs::write(marker, "go").expect("marker");
	}
	for w in workers {
		w.join()
			.expect("commit thread")
			.expect("the held commit lands once released");
	}
	assert_eq!(
		answered,
		Ok(true),
		"a status read waited behind {POOL_SIZE} commits held by their hooks"
	);
	drop(repos);
}
