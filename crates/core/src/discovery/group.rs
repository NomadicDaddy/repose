//! Attaches worktrees to their main repository and sorts the result by name.

use std::path::Path;

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
	for (main, mut worktree) in worktrees {
		let k = key(&main);
		let owner = repos
			.iter()
			.position(|r| key(&r.path) == k)
			.or_else(|| same_folder(&repos, &main));
		match owner {
			Some(i) => {
				respell(&mut worktree, &main, &repos[i].path);
				repos[i].worktrees.push(worktree);
			}
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

/// Writes a worktree's main path and gitdir with its owner's spelling. `main` came from git's
/// gitdir (`<main>/.git/worktrees/<name>`), so the gitdir sits under it; the watcher reports events
/// under the walked spelling, and a worktree's index must map back to the worktree, not its owner.
fn respell(worktree: &mut RepoSummary, main: &Path, owner: &Path) {
	worktree.main_path = Some(owner.to_path_buf());
	if let Some(rest) = worktree
		.gitdir
		.as_deref()
		.and_then(|g| g.strip_prefix(main).ok())
	{
		worktree.gitdir = Some(owner.join(rest));
	}
}

/// The repository that is the folder `main` on disk, when the text of the paths differs. git
/// writes a worktree's gitdir with the folder's long name, while the walk keeps the root as
/// configured, which may be an 8.3 short name (a CI runner's temp folder is one) or reach the
/// folder through a junction. A path that cannot be resolved (gone, or no access) matches nothing.
fn same_folder(repos: &[RepoSummary], main: &Path) -> Option<usize> {
	let real = key(&std::fs::canonicalize(main).ok()?);
	repos
		.iter()
		.position(|r| std::fs::canonicalize(&r.path).is_ok_and(|p| key(&p) == real))
}
