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
	commit_cancellable(repo, message, None).map(|_| ())
}

/// What a commit that returned Ok did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Committed {
	/// The commit landed, then Cancel stopped a later hook (post-commit); the user must not be
	/// told nothing happened (Roger #31030 on a31e63b).
	pub hook_stopped: bool,
	/// With `hook_stopped`: whether the hook's process tree was stopped, or taskkill failed and
	/// its processes may still be running (Roger 5Q-1 on 08c6f17). True when nothing was stopped.
	pub tree_killed: bool,
}

/// `commit` with the user's Cancel: while a hook runs, Cancel kills git and the hook's whole
/// process tree. If HEAD had not moved, the commit did not happen and the index is left as git
/// left it, lock and all, which the caller reports (stage-and-commit clause 9); if HEAD had
/// moved, the commit landed and a later hook was stopped, which is Ok with `hook_stopped`. That
/// claim needs two trustworthy HEAD reads; a read that failed before or after keeps the answer
/// at Cancelled rather than guess (Roger 5Q-2 on 08c6f17).
pub fn commit_cancellable(
	repo: &Path,
	message: &str,
	cancel: Option<&Cancel>,
) -> Result<Committed, GitError> {
	let before = head_hash(repo);
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
	let result = run_cancellable(repo, Verb::Commit, &["-F", &path], cancel);
	let _ = std::fs::remove_file(&file);
	match result {
		Ok(_) => Ok(Committed {
			hook_stopped: false,
			tree_killed: true,
		}),
		Err(GitError::Cancelled {
			stderr_tail,
			tree_killed,
		}) => match landed_ours(repo, &before, message) {
			Landed::Ours => Ok(Committed {
				hook_stopped: true,
				tree_killed,
			}),
			Landed::No => Err(GitError::Cancelled {
				stderr_tail,
				tree_killed,
			}),
			Landed::Unknown => Err(GitError::OutcomeUnknown { tree_killed }),
		},
		Err(e) => Err(e),
	}
}

/// What HEAD says after a cancelled commit.
enum Landed {
	/// HEAD is the commit this call made.
	Ours,
	/// HEAD is where it was: nothing landed.
	No,
	/// A read failed, or HEAD moved in a way that neither proves nor rules out this commit, so
	/// neither answer can be given (Roger 5Q-1 on 28883f9, #31066 on ca04ed1).
	Unknown,
}

/// The commit this call made is HEAD now only when both HEAD reads worked, HEAD moved, the new
/// commit's first parent is the old HEAD (or it has none, when the branch was unborn) and its
/// message is the one given. A teammate's commit or a branch switch in that checkout during a
/// long hook fails one of those (Morticia L-3 on 08c6f17, Roger 5Q-2 on 28883f9); the window
/// is the whole hook run, not the moment of the cancel.
fn landed_ours(repo: &Path, before: &Result<Option<String>, GitError>, message: &str) -> Landed {
	let (Ok(old), Ok(after)) = (before, head_hash(repo)) else {
		return Landed::Unknown;
	};
	let Some(new) = after else {
		return Landed::No;
	};
	if old.as_deref() == Some(new.as_str()) {
		return Landed::No;
	}
	let parent = match run(repo, Verb::RevParse, &["--verify", "--quiet", "HEAD^"]) {
		Ok(out) => Some(String::from_utf8_lossy(&out).trim().to_string()),
		Err(GitError::Failed { code: Some(1), .. }) => None,
		Err(_) => return Landed::Unknown,
	};
	// HEAD moved, but not straight from the old HEAD: our commit may sit under a later one made
	// by someone else during a long post-commit hook, so its absence is not established and the
	// answer is Unknown, not No (Roger #31066 on ca04ed1).
	if parent != *old {
		return Landed::Unknown;
	}
	// git tidies a message (trailing spaces go, runs of blank lines become one) and a
	// commit-msg hook may edit it, so the comparison is on the tidied form, and a commit that
	// sits on the old HEAD but reads differently is Unknown, not No (Morticia L-4 on 4f22cd3).
	match run(repo, Verb::Log, &["-1", "--format=%B", "HEAD"]) {
		Ok(body) if tidy(&String::from_utf8_lossy(&body)) == tidy(message) => Landed::Ours,
		Ok(_) => Landed::Unknown,
		Err(_) => Landed::Unknown,
	}
}

/// A message as git stores it: no trailing spaces on a line, no run of blank lines, trimmed.
fn tidy(message: &str) -> String {
	let mut out = Vec::new();
	let mut blank = false;
	for line in message.lines().map(str::trim_end) {
		if line.is_empty() {
			if !blank && !out.is_empty() {
				out.push("");
			}
			blank = true;
		} else {
			out.push(line);
			blank = false;
		}
	}
	out.join("\n").trim().to_string()
}

/// HEAD's hash: Ok(None) only for an unborn branch (exit 1 with nothing on stderr, as
/// head_state reads it); a timeout, lost output or an unreadable ref is an error, so a
/// comparison of two reads never rests on a failed one.
fn head_hash(repo: &Path) -> Result<Option<String>, GitError> {
	match run(repo, Verb::RevParse, &["--verify", "--quiet", "HEAD"]) {
		Ok(out) => Ok(Some(String::from_utf8_lossy(&out).trim().to_string())),
		Err(GitError::Failed {
			code: Some(1),
			ref stderr_tail,
		}) if stderr_tail.trim().is_empty() => Ok(None),
		Err(e) => Err(e),
	}
}

/// Where this repository's index lock lives, which for a worktree is under the main repository's
/// `.git/worktrees/<name>/`.
pub fn index_lock_path(repo: &Path) -> Option<PathBuf> {
	let out = run(repo, Verb::RevParse, &["--git-path", "index.lock"]).ok()?;
	let text = String::from_utf8_lossy(&out).trim().to_string();
	let p = PathBuf::from(text);
	Some(if p.is_absolute() { p } else { repo.join(p) })
}
