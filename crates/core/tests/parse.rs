//! Assertion A5: the porcelain v2 parser against real git output: staged, unstaged, untracked,
//! a rename with both paths, a conflict, ahead and behind from branch.ab, detached and unborn.
#![allow(clippy::expect_used)]

mod support;

use repose_core::git::parse::EntryKind;
use repose_core::git::read_status;
use repose_core::model::BranchState;
use support::TempRepo;

#[test]
fn ordinary_changes_untracked_and_rename() {
	let repo = TempRepo::new("parse");
	repo.write("a.txt", "a\n");
	repo.write("old.txt", "same content for the rename detector\n");
	repo.commit_all("first");
	repo.write("a.txt", "a changed\n");
	repo.write("new-file.txt", "untracked\n");
	repo.write("staged.txt", "staged\n");
	repo.git(&["add", "staged.txt"]);
	repo.git(&["mv", "old.txt", "renamed.txt"]);

	let p = read_status(&repo.path).expect("status");
	assert_eq!(p.branch_state(), BranchState::Branch("main".into()));
	assert!(p.short_hash().map(|h| h.len() == 7).unwrap_or(false));
	let s = p.summary();
	assert_eq!(s.untracked, 1);
	assert_eq!(s.unstaged, 1, "a.txt modified in the worktree");
	assert_eq!(s.staged, 2, "staged.txt added and the rename");
	let rename = p
		.entries
		.iter()
		.find(|e| matches!(e.kind, EntryKind::Renamed { .. }))
		.expect("a rename entry");
	assert_eq!(rename.path, "renamed.txt");
	match &rename.kind {
		EntryKind::Renamed { from, score, .. } => {
			assert_eq!(from, "old.txt");
			assert!(score.starts_with('R'), "score {score}");
		}
		other => panic!("unexpected {other:?}"),
	}
	assert!(
		s.upstream.is_none() && s.ahead.is_none(),
		"no upstream, no counts"
	);
}

#[test]
fn conflicts_are_counted() {
	let repo = TempRepo::new("conflict");
	repo.write("c.txt", "base\n");
	repo.commit_all("base");
	repo.git(&["checkout", "-q", "-b", "other"]);
	repo.write("c.txt", "theirs\n");
	repo.commit_all("theirs");
	repo.git(&["checkout", "-q", "main"]);
	repo.write("c.txt", "ours\n");
	repo.commit_all("ours");
	repo.git(&["merge", "-q", "other"]);
	let p = read_status(&repo.path).expect("status");
	assert_eq!(p.summary().conflicts, 1);
	assert!(
		p.entries
			.iter()
			.any(|e| matches!(e.kind, EntryKind::Unmerged { .. }) && e.path == "c.txt")
	);
}

#[test]
fn ahead_and_behind_come_from_branch_ab() {
	let upstream = TempRepo::new("upstream");
	upstream.write("u.txt", "1\n");
	upstream.commit_all("one");
	let clone = TempRepo::new("clone");
	std::fs::remove_dir_all(clone.path.join(".git")).expect("clear");
	let src = upstream.path.to_string_lossy().replace('\\', "/");
	let dst = clone.path.to_string_lossy().to_string();
	upstream.git(&["clone", "-q", &src, &dst]);
	clone.write("local.txt", "l\n");
	clone.commit_all("local");
	upstream.write("u.txt", "2\n");
	upstream.commit_all("two");
	clone.git(&["fetch", "-q"]);
	let p = read_status(&clone.path).expect("status");
	assert_eq!(p.upstream.as_deref(), Some("origin/main"));
	assert_eq!(p.ahead, Some(1));
	assert_eq!(p.behind, Some(1));
}

#[test]
fn unborn_and_detached() {
	let unborn = TempRepo::new("unborn-status");
	let p = read_status(&unborn.path).expect("status");
	assert_eq!(p.branch_state(), BranchState::Unborn);
	assert!(p.short_hash().is_none());

	let detached = TempRepo::new("detached-status");
	detached.write("d.txt", "d\n");
	detached.commit_all("d");
	let head = detached.git(&["rev-parse", "HEAD"]);
	detached.git(&["checkout", "-q", "--detach", head.trim()]);
	let p = read_status(&detached.path).expect("status");
	assert_eq!(p.branch_state(), BranchState::Detached);
}
