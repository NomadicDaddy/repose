//! Which repository an event path belongs to: the longest repository or worktree root that
//! prefixes it, except that a path under a worktree's `<main>/.git/worktrees/<name>/` belongs
//! to that worktree, since its HEAD and index live there and not under its own checkout.

use std::path::{Component, Path, PathBuf};

use crate::model::RepoSummary;

/// One owner of file-system events.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Owner {
	/// The repository or worktree path the refresh is for.
	repo: PathBuf,
	/// The folder whose descendants belong to it, lower-cased components for the comparison.
	prefix: Vec<String>,
	/// Events here are reported as `.git/<rest>`: the worktree's gitdir.
	is_gitdir: bool,
}

#[derive(Debug, Default, Clone)]
pub struct RepoMap {
	/// Longest prefix first, gitdirs ahead of checkouts of the same length.
	owners: Vec<Owner>,
}

/// An event mapped to its repository and its slash-separated path inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapped {
	pub repo: PathBuf,
	pub rel: String,
}

/// The path's prefix and normal components in their original spelling, with `.` ignored and
/// each `..` removing the normal component before it (never the prefix), so
/// `thing\..\thingamajig` is `thingamajig` and not a path inside `thing`.
fn resolved(path: &Path) -> Vec<String> {
	let mut parts: Vec<String> = Vec::new();
	let mut normals = 0;
	for c in path.components() {
		match c {
			Component::Prefix(p) => parts.push(p.as_os_str().to_string_lossy().to_string()),
			Component::Normal(s) => {
				parts.push(s.to_string_lossy().to_string());
				normals += 1;
			}
			Component::ParentDir if normals > 0 => {
				parts.pop();
				normals -= 1;
			}
			_ => {}
		}
	}
	parts
}

/// The resolved components lower-cased, for comparison only.
fn components(path: &Path) -> Vec<String> {
	resolved(path)
		.into_iter()
		.map(|s| s.to_lowercase())
		.collect()
}

impl RepoMap {
	pub fn new(repos: &[RepoSummary]) -> Self {
		let mut owners = Vec::new();
		for r in repos {
			for row in std::iter::once(r).chain(r.worktrees.iter()) {
				owners.push(Owner {
					repo: row.path.clone(),
					prefix: components(&row.path),
					is_gitdir: false,
				});
				if let Some(gitdir) = &row.gitdir {
					owners.push(Owner {
						repo: row.path.clone(),
						prefix: components(gitdir),
						is_gitdir: true,
					});
				}
			}
		}
		owners.sort_by(|a, b| {
			b.prefix
				.len()
				.cmp(&a.prefix.len())
				.then(b.is_gitdir.cmp(&a.is_gitdir))
		});
		Self { owners }
	}

	/// None for a path under no known repository.
	pub fn owner(&self, path: &Path) -> Option<Mapped> {
		let parts = components(path);
		let owner = self
			.owners
			.iter()
			.find(|o| parts.len() >= o.prefix.len() && parts[..o.prefix.len()] == o.prefix[..])?;
		// Keep the original spelling of the tail; only the comparison was lower-cased.
		let tail: Vec<String> = resolved(path)
			.into_iter()
			.skip(owner.prefix.len())
			.collect();
		let mut rel = tail.join("/");
		if owner.is_gitdir {
			rel = if rel.is_empty() {
				".git".to_string()
			} else {
				format!(".git/{rel}")
			};
		}
		Some(Mapped {
			repo: owner.repo.clone(),
			rel,
		})
	}
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
	use super::*;
	use crate::model::RepoKind;

	#[test]
	fn longest_prefix_wins_and_a_worktree_gitdir_maps_to_the_worktree() {
		let mut main = RepoSummary::new(PathBuf::from("D:\\apps\\thing"), RepoKind::Repository);
		let mut wt = RepoSummary::new(
			PathBuf::from("D:\\apps.worktrees\\thing\\task"),
			RepoKind::Worktree,
		);
		wt.main_path = Some(main.path.clone());
		wt.gitdir = Some(PathBuf::from("D:\\apps\\thing\\.git\\worktrees\\task"));
		main.worktrees.push(wt);
		let nested = RepoSummary::new(
			PathBuf::from("D:\\apps\\thing\\vendor\\lib"),
			RepoKind::Repository,
		);
		let map = RepoMap::new(&[main, nested]);

		let m = map
			.owner(Path::new("D:\\apps\\thing\\src\\Main.rs"))
			.expect("under main");
		assert_eq!(
			(m.repo.to_str(), m.rel.as_str()),
			(Some("D:\\apps\\thing"), "src/Main.rs")
		);

		let m = map
			.owner(Path::new("D:\\apps\\thing\\src\\..\\src\\Main.rs"))
			.expect("a resolved .. stays inside");
		assert_eq!(
			(m.repo.to_str(), m.rel.as_str()),
			(Some("D:\\apps\\thing"), "src/Main.rs")
		);
		assert_eq!(
			map.owner(Path::new("D:\\apps\\thing\\..\\thingamajig\\x")),
			None,
			"a .. that leaves the repository is not owned by it"
		);

		let m = map
			.owner(Path::new("d:\\APPS\\thing\\.git\\index"))
			.expect("case-insensitive");
		assert_eq!(m.rel, ".git/index");

		let m = map
			.owner(Path::new("D:\\apps\\thing\\.git\\worktrees\\task\\index"))
			.expect("worktree gitdir");
		assert_eq!(
			(m.repo.to_str(), m.rel.as_str()),
			(Some("D:\\apps.worktrees\\thing\\task"), ".git/index"),
			"the worktree's index refreshes the worktree row, not the main repository"
		);

		let m = map
			.owner(Path::new("D:\\apps.worktrees\\thing\\task\\README.md"))
			.expect("worktree checkout");
		assert_eq!(m.repo.to_str(), Some("D:\\apps.worktrees\\thing\\task"));

		let m = map
			.owner(Path::new("D:\\apps\\thing\\vendor\\lib\\a.c"))
			.expect("nested repository");
		assert_eq!(m.repo.to_str(), Some("D:\\apps\\thing\\vendor\\lib"));

		assert!(map.owner(Path::new("D:\\elsewhere\\x")).is_none());
		assert!(
			map.owner(Path::new("D:\\apps\\thingamajig\\x")).is_none(),
			"a sibling that shares a name prefix is not under the repository"
		);
	}
}
