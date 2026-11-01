//! Walks the configured roots, classifies what it finds, and groups worktrees under their main
//! repository.

mod classify;
mod group;
mod walk;

pub use classify::{Found, classify};
pub use group::group;
pub use walk::walk;

use crate::config::Config;
use crate::model::RepoSummary;

/// One scan of every root. Repositories come back sorted by name with their worktrees attached.
/// Nothing here spawns git; branch and hash are read from `.git` files so the list paints fast.
/// A root that is missing is skipped here and named by `config::missing_roots`.
pub fn discover(config: &Config) -> Vec<RepoSummary> {
	let mut found = Vec::new();
	for root in &config.roots {
		walk(root, config.depth, &config.exclude, &mut found);
	}
	group(found)
}

/// Whether a directory name is pruned from the walk.
pub fn skipped(name: &str, exclude: &[String]) -> bool {
	name == ".git" || exclude.iter().any(|e| e == name)
}
