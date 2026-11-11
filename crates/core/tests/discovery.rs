//! Assertion A6: a repository, a linked worktree, a nested repository and a submodule come back as
//! two rows, one grouped worktree and a submodule count.
#![allow(clippy::expect_used)]

mod support;

use std::path::PathBuf;

use repose_core::config::Config;
use repose_core::discovery::discover;
use repose_core::model::{BranchState, RepoKind};
use support::{TempRepo, git_in};

#[test]
fn groups_worktrees_counts_submodules_and_lists_nested_repositories() {
	let root = TempRepo::new("root");
	// The root itself is not a repository; remove the .git init gave it.
	std::fs::remove_dir_all(root.path.join(".git")).expect("remove root .git");

	let main = root.path.join("main");
	std::fs::create_dir_all(&main).expect("main");
	git_in(&main, &["init", "-q", "-b", "main"]);
	std::fs::write(main.join("a.txt"), "a\n").expect("write");
	git_in(&main, &["add", "a.txt"]);
	git_in(&main, &["commit", "-q", "-m", "first"]);

	let wt = root.path.join("worktrees").join("main-feature");
	std::fs::create_dir_all(root.path.join("worktrees")).expect("worktrees dir");
	let wt_str = wt.to_string_lossy().to_string();
	git_in(&main, &["worktree", "add", "-q", "-b", "feature", &wt_str]);

	let nested = main.join("vendor").join("inner");
	std::fs::create_dir_all(&nested).expect("nested");
	git_in(&nested, &["init", "-q", "-b", "main"]);

	let sub_src = root.path.join("sub-src");
	std::fs::create_dir_all(&sub_src).expect("sub src");
	git_in(&sub_src, &["init", "-q", "-b", "main"]);
	std::fs::write(sub_src.join("s.txt"), "s\n").expect("write");
	git_in(&sub_src, &["add", "s.txt"]);
	git_in(&sub_src, &["commit", "-q", "-m", "sub"]);
	let sub_url = sub_src.to_string_lossy().replace('\\', "/");
	git_in(
		&main,
		&[
			"-c",
			"protocol.file.allow=always",
			"submodule",
			"add",
			"-q",
			&sub_url,
			"libs/sub",
		],
	);

	let config = Config {
		roots: vec![root.path.clone()],
		depth: 3,
		..Config::default()
	};
	let repos = discover(&config);
	let names: Vec<&str> = repos.iter().map(|r| r.name.as_str()).collect();

	assert!(names.contains(&"main"), "main repository listed: {names:?}");
	assert!(
		names.contains(&"inner"),
		"nested repository is its own row: {names:?}"
	);
	assert!(
		names.contains(&"sub-src"),
		"the submodule source repository: {names:?}"
	);
	assert!(
		!names.contains(&"sub"),
		"submodule paths are not rows: {names:?}"
	);
	assert!(
		!names.contains(&"main-feature"),
		"worktrees are grouped, not top-level: {names:?}"
	);

	let main_row = repos.iter().find(|r| r.name == "main").expect("main row");
	assert_eq!(main_row.kind, RepoKind::Repository);
	assert_eq!(main_row.branch, BranchState::Branch("main".into()));
	assert_eq!(main_row.submodules, 1);
	assert_eq!(main_row.worktrees.len(), 1);
	let wt_row = &main_row.worktrees[0];
	assert_eq!(wt_row.kind, RepoKind::Worktree);
	assert_eq!(wt_row.branch, BranchState::Branch("feature".into()));
	assert_eq!(wt_row.main_path.as_deref(), Some(main.as_path()));
	assert!(
		wt_row.short_hash.is_some(),
		"worktree hash read from the main repository's refs"
	);
}

/// A root reached through another spelling of its folder (here a junction; on a CI runner the
/// temp folder's 8.3 short name) still groups a worktree under its repository, and the
/// worktree's main path and gitdir use the root's spelling, so watcher events map back to it.
#[test]
fn a_root_spelled_another_way_still_groups_its_worktrees() {
	let real = TempRepo::new("spelled");
	std::fs::remove_dir_all(real.path.join(".git")).expect("remove root .git");
	let main = real.path.join("main");
	std::fs::create_dir_all(&main).expect("main");
	git_in(&main, &["init", "-q", "-b", "main"]);
	std::fs::write(main.join("a.txt"), "a\n").expect("write");
	git_in(&main, &["add", "a.txt"]);
	git_in(&main, &["commit", "-q", "-m", "first"]);
	let wt = real.path.join("main-feature");
	git_in(
		&main,
		&[
			"worktree",
			"add",
			"-q",
			"-b",
			"feature",
			&wt.to_string_lossy(),
		],
	);

	let via = real.path.with_extension("via");
	let made = std::process::Command::new("cmd")
		.args(["/C", "mklink", "/J"])
		.arg(&via)
		.arg(&real.path)
		.output()
		.expect("cmd runs");
	assert!(made.status.success(), "junction: {made:?}");
	let repos = discover(&Config {
		roots: vec![via.clone()],
		depth: 2,
		..Config::default()
	});
	let names: Vec<&str> = repos.iter().map(|r| r.name.as_str()).collect();
	assert_eq!(names, ["main"], "the worktree is grouped, not its own row");
	let wt_row = &repos[0].worktrees[0];
	assert_eq!(
		wt_row.main_path.as_deref(),
		Some(via.join("main").as_path())
	);
	let gitdir = wt_row.gitdir.as_deref().expect("gitdir");
	assert!(gitdir.starts_with(&via), "{}", gitdir.display());
	assert!(gitdir.join("HEAD").is_file(), "{}", gitdir.display());
	// Removing the junction leaves its target alone; the temp repository then removes the rest.
	std::fs::remove_dir(&via).expect("remove the junction");
}

#[test]
fn depth_is_bounded() {
	let root = TempRepo::new("depth");
	std::fs::remove_dir_all(root.path.join(".git")).expect("remove root .git");
	let shallow = root.path.join("a").join("b").join("c");
	let deep = root.path.join("a").join("b").join("c").join("d");
	for p in [&shallow, &deep] {
		std::fs::create_dir_all(p).expect("dirs");
	}
	git_in(&shallow, &["init", "-q"]);
	git_in(&deep, &["init", "-q"]);
	let config = Config {
		roots: vec![root.path.clone()],
		depth: 3,
		..Config::default()
	};
	let names: Vec<String> = discover(&config).into_iter().map(|r| r.name).collect();
	assert_eq!(names, vec!["c".to_string()], "depth 3 finds c and not d");
}

#[test]
fn unborn_and_detached_heads_are_named() {
	let unborn = TempRepo::new("unborn");
	let detached = TempRepo::new("detached");
	detached.write("x.txt", "x\n");
	detached.commit_all("one");
	let head = detached.git(&["rev-parse", "HEAD"]);
	detached.git(&["checkout", "-q", "--detach", head.trim()]);
	let config = Config {
		roots: vec![unborn.path.clone(), detached.path.clone()],
		depth: 0,
		..Config::default()
	};
	let repos = discover(&config);
	let by_path = |p: &PathBuf| repos.iter().find(|r| &r.path == p).expect("row");
	assert_eq!(by_path(&unborn.path).branch, BranchState::Unborn);
	assert_eq!(by_path(&unborn.path).branch_line(), "no commits");
	assert_eq!(by_path(&detached.path).branch, BranchState::Detached);
	assert!(
		by_path(&detached.path)
			.branch_line()
			.starts_with("detached · ")
	);
}
