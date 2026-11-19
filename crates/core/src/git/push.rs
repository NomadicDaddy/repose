//! Push (remote-pull-push, A1 as revised by Phillip #58501): the checked-out branch to the
//! upstream it tracks, as a fast-forward of the commit repose last saw there, and nothing else.

use std::path::Path;

use super::commands::{Cancel, GitError, Verb, run, run_cancellable};

/// `git push --force-with-lease=<merge ref>:<seen> <upstream remote> HEAD:<merge ref>`, on the
/// user's Push only.
///
/// - The explicit remote and refspec keep remote.<name>.push, pushRemote, pushDefault and mirror
///   from sending anything else (Roger 5Q-1, Morticia M on f61983d).
/// - The lease makes the server refuse unless its branch is still exactly `<seen>`, the commit
///   the saved tracking ref names: a branch deleted on the server since the last fetch is not
///   created again, and one someone else moved is not overwritten (Roger 5Q-3 on 2d1efdd).
/// - A lease alone would let a non-fast-forward through, so HEAD must contain `<seen>` first; the
///   push is then always a fast-forward, never a force.
///
/// A refusal, a sign-in request or a network failure comes back as the error with git's own
/// message, and nothing local changes. Cancel stops git and anything it started.
pub fn push(repo: &Path, cancel: Option<&Cancel>) -> Result<(), GitError> {
	let target = upstream_of_head(repo)?;
	let behind = run(
		repo,
		Verb::Log,
		&["-1", "--format=%H", &target.seen, "^HEAD", "--"],
	)?;
	if !behind.is_empty() {
		return Err(refused(
			"This branch is behind its upstream; Pull first, or merge it in your own tools",
		));
	}
	let lease = format!("--force-with-lease={}:{}", target.merge, target.seen);
	let refspec = format!("HEAD:{}", target.merge);
	run_cancellable(
		repo,
		Verb::Push,
		&[&lease, &target.remote, &refspec],
		cancel,
	)
	.map(|_| ())
}

struct Target {
	remote: String,
	merge: String,
	seen: String,
}

fn refused(why: &str) -> GitError {
	GitError::Spawn(why.to_string())
}

/// The checked-out branch's upstream remote and merge ref (branch.<name>.remote and .merge) as
/// git resolves them, and the commit its saved tracking ref names. A detached HEAD, no upstream,
/// a merge ref outside refs/heads or a missing tracking ref is an error, never a guess.
fn upstream_of_head(repo: &Path) -> Result<Target, GitError> {
	let head = run(repo, Verb::SymbolicRef, &["HEAD"])
		.map_err(|_| refused("Push needs a checked-out branch"))?;
	let head = String::from_utf8_lossy(&head).trim().to_string();
	let format = "--format=%(upstream:remotename)%00%(upstream:remoteref)%00%(upstream)";
	let out = run(repo, Verb::ForEachRef, &[format, &head])?;
	let out = String::from_utf8_lossy(&out);
	let mut lines = out.lines();
	let (Some(line), None) = (lines.next(), lines.next()) else {
		return Err(refused("This branch has no upstream"));
	};
	let mut parts = line.split('\0');
	let (Some(remote), Some(merge), Some(tracking), None) =
		(parts.next(), parts.next(), parts.next(), parts.next())
	else {
		return Err(refused("This branch has no upstream"));
	};
	if remote.is_empty() || !merge.starts_with("refs/heads/") || tracking.is_empty() {
		return Err(refused("This branch has no upstream"));
	}
	let seen = run(
		repo,
		Verb::RevParse,
		&["--verify", "--quiet", &format!("{tracking}^{{commit}}")],
	)
	.map_err(|_| refused("This branch's upstream is gone from the remote"))?;
	Ok(Target {
		remote: remote.to_string(),
		merge: merge.to_string(),
		seen: String::from_utf8_lossy(&seen).trim().to_string(),
	})
}
