//! Attaches worktrees to their main repository and sorts the result by name.

use super::classify::Found;
use crate::model::{RepoSummary, path_key as key};

pub fn group(found: Vec<Found>) -> Vec<RepoSummary> {
	let mut repos: Vec<RepoSummary> = Vec::new();
	let mut worktrees = Vec::new();
	for item in found {
		match item {
			Found::Repository(r) => repos.push(r),
			Found::Worktree { main, summary } => worktrees.push((main, summary)),
		}
	}
	repos.sort_by_key(|r| key(&r.path));
	repos.dedup_by_key(|r| key(&r.path));
	for (main, worktree) in worktrees {
		let k = key(&main);
		match repos.iter_mut().find(|r| key(&r.path) == k) {
			Some(owner) => owner.worktrees.push(worktree),
			None => {
				// The main repository is outside every root: keep the worktree as its own row.
				repos.push(worktree);
			}
		}
	}
	for repo in &mut repos {
		repo.worktrees.sort_by_key(|w| w.name.to_lowercase());
	}
	repos.sort_by_key(|r| r.name.to_lowercase());
	repos
}
