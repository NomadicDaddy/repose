//! The watcher against the real OS: a repository and a linked worktree under one root; a write
//! in the main checkout refreshes the main row, a write to the worktree's own index (which lives
//! under <main>/.git/worktrees/<name>/) refreshes the worktree row and not the main one, and a
//! write under target/ refreshes nothing.
#![allow(clippy::expect_used)]

mod support;

use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use repose_core::config::Config;
use repose_core::discovery::discover;
use repose_core::watch::{Refresh, RepoMap, Watch};
use support::TempRepo;

const SETTLE: Duration = Duration::from_secs(4);

fn next_refresh(rx: &std::sync::mpsc::Receiver<Refresh>) -> Option<Refresh> {
	rx.recv_timeout(SETTLE).ok()
}

fn drain(rx: &std::sync::mpsc::Receiver<Refresh>) {
	while rx.try_recv().is_ok() {}
	std::thread::sleep(Duration::from_millis(400));
	while rx.try_recv().is_ok() {}
}

#[test]
fn events_refresh_the_owning_row_and_a_worktree_index_refreshes_the_worktree() {
	// One temp root holding the main repository and its worktree side by side.
	let root = TempRepo::new("watch-root");
	let main = root.path.join("main");
	std::fs::create_dir_all(&main).expect("main dir");
	support::git_in(&main, &["init", "-q", "-b", "main"]);
	support::git_in(&main, &["config", "core.autocrlf", "false"]);
	std::fs::write(main.join("a.txt"), "a\n").expect("write");
	support::git_in(&main, &["add", "a.txt"]);
	support::git_in(&main, &["commit", "-q", "-m", "base"]);
	let wt = root.path.join("task");
	let wt_arg = wt.to_string_lossy().to_string();
	support::git_in(&main, &["worktree", "add", "-q", &wt_arg, "-b", "task"]);

	let config = Config {
		roots: vec![root.path.clone()],
		depth: 2,
		..Config::default()
	};
	let repos = discover(&config);
	let main_row = repos
		.iter()
		.find(|r| r.path == main)
		.expect("main discovered");
	let wt_row = main_row
		.worktrees
		.iter()
		.find(|w| w.path == wt)
		.expect("worktree discovered under main");
	let gitdir = wt_row.gitdir.clone().expect("worktree gitdir recorded");
	assert!(gitdir.join("index").is_file(), "{}", gitdir.display());

	let (tx, rx) = channel();
	let (watch, unwatched) = Watch::start(
		&config.roots,
		config.exclude.clone(),
		RepoMap::new(&repos),
		tx,
		Box::new(|| {}),
	)
	.expect("watcher starts");
	assert!(unwatched.is_empty(), "{unwatched:?}");
	drain(&rx);

	// A write in the main checkout.
	std::fs::write(main.join("a.txt"), "a2\n").expect("write");
	assert_eq!(
		next_refresh(&rx),
		Some(Refresh::Repo(main.clone())),
		"the main row refreshes"
	);
	drain(&rx);

	// A write to the worktree's index, which sits under the main repository's .git folder.
	support::git_in(&wt, &["add", "-A"]);
	std::fs::write(wt.join("b.txt"), "b\n").expect("write");
	support::git_in(&wt, &["add", "b.txt"]);
	let started = Instant::now();
	let mut seen: Vec<PathBuf> = Vec::new();
	while started.elapsed() < SETTLE {
		match rx.recv_timeout(Duration::from_millis(500)) {
			Ok(Refresh::Repo(p)) => seen.push(p),
			Ok(Refresh::All) => panic!("overflow during a small test"),
			Err(_) => {
				if !seen.is_empty() {
					break;
				}
			}
		}
	}
	assert!(seen.contains(&wt), "the worktree row refreshes: {seen:?}");
	assert!(
		!seen.contains(&main),
		"the main repository does not refresh for the worktree's index: {seen:?}"
	);
	drain(&rx);

	// Build output is ignored.
	std::fs::create_dir_all(main.join("target")).expect("target dir");
	std::fs::write(main.join("target").join("out.bin"), "x").expect("write");
	assert_eq!(
		rx.recv_timeout(Duration::from_millis(1500)).ok(),
		None,
		"nothing under target/ refreshes"
	);
	drop(watch);
}
