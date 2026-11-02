//! The branch list against real git: the current branch first, upstream counts from saved refs,
//! a branch with no upstream, a detached HEAD and an unborn repository. No fetch anywhere.
#![allow(clippy::expect_used)]

mod support;

use repose_core::git::branches::read_branches;
use repose_core::model::HeadState;
use support::TempRepo;

#[test]
fn current_first_then_by_name_with_saved_counts() {
	let repo = TempRepo::new("branches");
	repo.write("base.txt", "base\n");
	repo.commit_all("base");
	// "up" stands in for a remote-tracking ref: main tracks it and is one commit ahead.
	repo.git(&["branch", "up"]);
	repo.git(&["branch", "zeta"]);
	repo.git(&["branch", "alpha"]);
	repo.write("mine.txt", "mine\n");
	repo.commit_all("mine");
	repo.git(&["branch", "-u", "up"]);
	// "level" tracks "up" and sits on the same commit: counts of zero, not unknown (Roger on
	// 0de88a1).
	repo.git(&["branch", "level", "up"]);
	repo.git(&["branch", "-u", "up", "level"]);
	let branches = read_branches(&repo.path).expect("branches");
	assert_eq!(branches.head, HeadState::OnBranch("main".into()));
	let names: Vec<&str> = branches.list.iter().map(|b| b.name.as_str()).collect();
	assert_eq!(
		names,
		vec!["main", "alpha", "level", "up", "zeta"],
		"current first, then by name"
	);
	let level = branches
		.list
		.iter()
		.find(|b| b.name == "level")
		.expect("level");
	assert_eq!(level.upstream.as_deref(), Some("up"));
	assert_eq!((level.ahead, level.behind), (Some(0), Some(0)));
	let main = &branches.list[0];
	assert!(main.current);
	assert_eq!(main.upstream.as_deref(), Some("up"));
	assert_eq!((main.ahead, main.behind), (Some(1), Some(0)));
	assert_eq!(main.short_hash.len(), 7, "short hash {:?}", main.short_hash);
	let alpha = &branches.list[1];
	assert!(!alpha.current);
	assert_eq!(alpha.upstream, None);
	assert_eq!((alpha.ahead, alpha.behind), (None, None));
}

#[test]
fn detached_and_unborn_heads_are_named() {
	let repo = TempRepo::new("branches-detached");
	repo.write("a.txt", "a\n");
	repo.commit_all("first");
	let hash = repo.git(&["rev-parse", "--short", "HEAD"]);
	repo.git(&["checkout", "-q", "--detach"]);
	let branches = read_branches(&repo.path).expect("branches");
	assert_eq!(branches.head, HeadState::Detached(hash.trim().to_string()));
	assert_eq!(branches.list.len(), 1);
	assert!(
		!branches.list[0].current,
		"no branch is current while detached"
	);

	let empty = TempRepo::new("branches-unborn");
	let branches = read_branches(&empty.path).expect("branches of an unborn repository");
	assert_eq!(branches.head, HeadState::Unborn);
	assert!(branches.list.is_empty());
}
