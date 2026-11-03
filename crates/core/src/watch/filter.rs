//! Which file-system events deserve a refresh: anything in the working tree outside the build
//! and dependency folders, and under `.git` only the few files that mean HEAD, the index or a ref
//! moved. Everything else git writes while it works (objects, lock files, logs of other refs)
//! is noise that would only make the refresh fire more often.

use crate::config::name_excluded;

/// `rel` is the path inside the repository, slash-separated (`.git/index`, `src/main.rs`), or
/// inside a worktree's gitdir prefixed with `.git/` by the map. `exclude` is the config's list.
pub fn keep(rel: &str, exclude: &[String]) -> bool {
	let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
	let Some(first) = parts.first() else {
		return false;
	};
	if first.eq_ignore_ascii_case(".git") {
		return keep_in_git(&parts[1..]);
	}
	!parts.iter().any(|p| name_excluded(p, exclude))
}

/// HEAD, index, ORIG_HEAD, MERGE_HEAD, packed-refs, anything under refs/, logs/HEAD, and the same
/// three for a linked worktree under worktrees/<name>/. Git's own `.lock` files are dropped
/// here, and only here: in the working tree a `.lock` is a tracked file (Cargo.lock, bun.lock)
/// whose change must refresh (Morticia L-2 on 7e70f1f).
fn keep_in_git(parts: &[&str]) -> bool {
	if parts.last().is_some_and(|p| p.ends_with(".lock")) {
		return false;
	}
	matches!(
		parts,
		[] | ["HEAD" | "index" | "ORIG_HEAD" | "MERGE_HEAD" | "packed-refs"]
			| ["refs", ..]
			| ["logs", "HEAD"]
			| ["worktrees", _, "HEAD" | "index"]
			| ["worktrees", _, "logs", "HEAD"]
	)
}

#[cfg(test)]
mod tests {
	use super::keep;

	#[test]
	fn kept_and_dropped_paths() {
		let exclude: Vec<String> = ["node_modules", "target", "dist", ".prevers"]
			.iter()
			.map(|s| s.to_string())
			.collect();
		for path in [
			"src/main.rs",
			"README.md",
			".gitignore",
			".git",
			".git/HEAD",
			".git/index",
			".git/ORIG_HEAD",
			".git/MERGE_HEAD",
			".git/packed-refs",
			".git/refs/heads/main",
			".git/refs/remotes/origin/main",
			".git/logs/HEAD",
			".git/worktrees/task/HEAD",
			".git/worktrees/task/index",
			".git/worktrees/task/logs/HEAD",
			"docs/target.md",
			"Cargo.lock",
			"frontend/bun.lock",
			"src/tmp.lock",
		] {
			assert!(keep(path, &exclude), "kept: {path}");
		}
		for path in [
			"",
			"node_modules/left-pad/index.js",
			"frontend/node_modules/x/y.js",
			"target/debug/repose.exe",
			"dist/app.js",
			".prevers/old/file",
			".git/index.lock",
			".git/refs/heads/main.lock",
			".git/objects/ab/cdef",
			".git/logs/refs/heads/main",
			".git/FETCH_HEAD",
			".git/COMMIT_EDITMSG",
			".git/config",
			".git/worktrees/task/COMMIT_EDITMSG",
			".git/worktrees/task/logs/refs/heads/x",
			".git/worktrees/task/index.lock",
		] {
			assert!(!keep(path, &exclude), "dropped: {path}");
		}
	}

	#[test]
	fn an_exclude_entry_drops_a_path_whatever_its_case() {
		let exclude = vec!["node_modules".to_string()];
		assert!(!keep("Node_Modules/pkg/index.js", &exclude));
		assert!(keep("src/main.rs", &exclude));
	}
}
