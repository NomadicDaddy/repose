//! The write verbs: stage, unstage, discard and commit, through the installed git so hooks run.
//! Every call goes through `commands::run`, which sets GIT_LITERAL_PATHSPECS=1, so a path is
//! only ever the path named. Writes have no timeout.

use std::path::{Path, PathBuf};

use super::commands::{Cancel, GitError, Verb, run, run_cancellable};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Head {
	Present,
	/// No commit yet: `restore --staged` has no source, so unstaging drops the index entry.
	Unborn,
}

/// Three answers, never two: `rev-parse --verify --quiet HEAD` exits 0 when HEAD resolves and 1
/// with nothing on stderr when it does not. Anything else (a timeout, lost output, a repository
/// git cannot read) is an error and no write may be chosen from it. Morticia M-1 on 51bc93d: a
/// bool here sent `rm --cached -r .` at a committed repository after a timeout. Exit 1 alone is
/// still two cases (Morticia L-5 on cba0c7a): a branch with no commits, or a branch whose ref
/// file is corrupt. Unborn is claimed only when the ref file HEAD names does not exist.
fn head_state(repo: &Path) -> Result<Head, GitError> {
	match run(repo, Verb::RevParse, &["--verify", "--quiet", "HEAD"]) {
		Ok(_) => Ok(Head::Present),
		Err(GitError::Failed {
			code: Some(1),
			ref stderr_tail,
		}) if stderr_tail.trim().is_empty() => confirm_unborn(repo),
		Err(e) => Err(e),
	}
}

fn confirm_unborn(repo: &Path) -> Result<Head, GitError> {
	// On a corrupt branch ref git refuses HEAD here ("No such ref: HEAD", exit 128); that refusal
	// is the answer, said in the user's terms.
	let out = match run(repo, Verb::SymbolicRef, &["HEAD"]) {
		Ok(out) => out,
		Err(GitError::Failed { .. }) => return Err(GitError::UnreadableRef("HEAD".to_string())),
		Err(e) => return Err(e),
	};
	let branch_ref = String::from_utf8_lossy(&out).trim().to_string();
	if branch_ref.is_empty() {
		return Err(GitError::UnreadableRef("HEAD".to_string()));
	}
	let out = run(repo, Verb::RevParse, &["--git-path", &branch_ref])?;
	let rel = String::from_utf8_lossy(&out).trim().to_string();
	let file = PathBuf::from(&rel);
	let file = if file.is_absolute() {
		file
	} else {
		repo.join(file)
	};
	if file.exists() {
		return Err(GitError::UnreadableRef(branch_ref));
	}
	Ok(Head::Unborn)
}

pub fn unstage(repo: &Path, paths: &[String]) -> Result<(), GitError> {
	if paths.is_empty() {
		return Ok(());
	}
	match head_state(repo)? {
		Head::Present => run(repo, Verb::Restore, &with_paths(&["--staged"], paths)).map(|_| ()),
		Head::Unborn => run(repo, Verb::RmCached, &with_paths(&[], paths)).map(|_| ()),
	}
}

pub fn unstage_all(repo: &Path) -> Result<(), GitError> {
	match head_state(repo)? {
		Head::Present => run(repo, Verb::Restore, &["--staged", "--", "."]).map(|_| ()),
		Head::Unborn => run(repo, Verb::RmCached, &["-r", "--", "."]).map(|_| ()),
	}
}

/// Restores tracked files from the index and removes untracked ones. The caller has already
/// shown every one of these paths to the user.
pub fn discard(repo: &Path, tracked: &[String], untracked: &[String]) -> Result<(), GitError> {
	if !tracked.is_empty() {
		run(repo, Verb::Restore, &with_paths(&[], tracked))?;
	}
	if !untracked.is_empty() {
		run(repo, Verb::Clean, &with_paths(&["-f"], untracked))?;
		let left: Vec<String> = untracked
			.iter()
			.filter(|p| repo.join(p).exists())
			.cloned()
			.collect();
		if !left.is_empty() {
			return Err(GitError::StillPresent(left));
		}
	}
	Ok(())
}

/// Commits what is staged with `git commit -F <file>`. The message goes through a temporary file
/// so quoting never matters, hooks run, and `--no-verify` and `--amend` cannot be expressed.
pub fn commit(repo: &Path, message: &str) -> Result<(), GitError> {
	commit_cancellable(repo, message, None)
}

/// `commit` with the user's Cancel: while a hook runs, Cancel kills git and the hook's whole
/// process tree and the commit does not happen; the index is left as git left it, lock and all,
/// which the caller reports (stage-and-commit clause 9).
pub fn commit_cancellable(
	repo: &Path,
	message: &str,
	cancel: Option<&Cancel>,
) -> Result<(), GitError> {
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
	let result = run_cancellable(repo, Verb::Commit, &["-F", &path], cancel).map(|_| ());
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
