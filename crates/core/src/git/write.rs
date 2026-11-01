//! The write verbs: stage, unstage, discard and commit, through the installed git so hooks run.
//! Every call goes through `commands::run`, which sets GIT_LITERAL_PATHSPECS=1, so a path is
//! only ever the path named. Writes have no timeout.

use std::path::{Path, PathBuf};

use super::commands::{GitError, Verb, run};

fn with_paths<'a>(head: &[&'a str], paths: &'a [String]) -> Vec<&'a str> {
	let mut args: Vec<&str> = head.to_vec();
	args.push("--");
	args.extend(paths.iter().map(String::as_str));
	args
}

pub fn stage(repo: &Path, paths: &[String]) -> Result<(), GitError> {
	if paths.is_empty() {
		return Ok(());
	}
	run(repo, Verb::Add, &with_paths(&[], paths)).map(|_| ())
}

pub fn stage_all(repo: &Path) -> Result<(), GitError> {
	run(repo, Verb::Add, &["--all"]).map(|_| ())
}

pub fn unstage(repo: &Path, paths: &[String]) -> Result<(), GitError> {
	if paths.is_empty() {
		return Ok(());
	}
	run(repo, Verb::Restore, &with_paths(&["--staged"], paths)).map(|_| ())
}

pub fn unstage_all(repo: &Path) -> Result<(), GitError> {
	run(repo, Verb::Restore, &["--staged", "--", "."]).map(|_| ())
}

/// Restores tracked files from the index and removes untracked ones. The caller has already
/// shown every one of these paths to the user.
pub fn discard(repo: &Path, tracked: &[String], untracked: &[String]) -> Result<(), GitError> {
	if !tracked.is_empty() {
		run(repo, Verb::Restore, &with_paths(&[], tracked))?;
	}
	if !untracked.is_empty() {
		run(repo, Verb::Clean, &with_paths(&["-f"], untracked))?;
	}
	Ok(())
}

/// Commits what is staged with `git commit -F <file>`. The message goes through a temporary file
/// so quoting never matters, hooks run, and `--no-verify` and `--amend` cannot be expressed.
pub fn commit(repo: &Path, message: &str) -> Result<(), GitError> {
	let file = std::env::temp_dir().join(format!(
		"repose-commit-{}-{}.txt",
		std::process::id(),
		std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map(|d| d.as_nanos())
			.unwrap_or(0)
	));
	std::fs::write(&file, message).map_err(|e| GitError::Spawn(e.to_string()))?;
	let path = file.to_string_lossy().to_string();
	let result = run(repo, Verb::Commit, &["-F", &path]).map(|_| ());
	let _ = std::fs::remove_file(&file);
	result
}

/// Where this repository's index lock lives, which for a worktree is under the main repository's
/// `.git/worktrees/<name>/`.
pub fn index_lock_path(repo: &Path) -> Option<PathBuf> {
	let out = run(repo, Verb::RevParse, &["--git-path", "index.lock"]).ok()?;
	let text = String::from_utf8_lossy(&out).trim().to_string();
	let p = PathBuf::from(text);
	Some(if p.is_absolute() { p } else { repo.join(p) })
}
