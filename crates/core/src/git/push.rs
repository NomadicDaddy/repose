//! Push (remote-pull-push, A1 as revised by Phillip #58501): the checked-out branch to the
//! upstream it tracks, as a fast-forward of the commit repose last saw there, and nothing else.

use std::path::Path;

use super::commands::{Cancel, GitError, Verb, run, run_cancellable};

/// What one Push sends, read once from the branch's own refs and config by [`plan`]; [`send`]
/// uses exactly these values, so a checkout or ref update in between cannot change what goes.
#[derive(Debug, Clone)]
pub struct PushPlan {
	remote: String,
	merge: String,
	/// The commit the saved tracking ref names: what the server's branch must still be.
	seen: String,
	/// The branch's own commit at plan time, which contains `seen`: what is sent.
	source: String,
}

impl PushPlan {
	pub fn source(&self) -> &str {
		&self.source
	}
}

/// The user's Push: [`plan`], then [`send`].
pub fn push(repo: &Path, cancel: Option<&Cancel>) -> Result<(), GitError> {
	send(repo, &plan(repo)?, cancel)
}

/// `git push --force-with-lease=<merge ref>:<seen> <upstream remote> <source>:<merge ref>`.
///
/// - The explicit remote and refspec keep remote.<name>.push, pushRemote, pushDefault and mirror
///   from sending anything else (Roger 5Q-1, Morticia M on f61983d).
/// - The lease makes the server refuse unless its branch is still exactly `seen`: a branch deleted
///   on the server since the last fetch is not created again, and one someone else moved is not
///   overwritten (Roger 5Q-3 on 2d1efdd).
/// - A lease alone would let a non-fast-forward through, so [`plan`] checked that `source`
///   contains `seen`, and the refspec sends that same commit id, never HEAD read again (Roger 5Q-4
///   on c50d643): the push is always a fast-forward, never a force.
///
/// A refusal, a sign-in request or a network failure comes back as the error with git's own
/// message, and nothing local changes. Cancel stops git and anything it started.
pub fn send(repo: &Path, plan: &PushPlan, cancel: Option<&Cancel>) -> Result<(), GitError> {
	let lease = format!("--force-with-lease={}:{}", plan.merge, plan.seen);
	let refspec = format!("{}:{}", plan.source, plan.merge);
	run_cancellable(repo, Verb::Push, &[&lease, &plan.remote, &refspec], cancel).map(|_| ())
}

fn refused(why: &str) -> GitError {
	GitError::Spawn(why.to_string())
}

fn commit_of(repo: &Path, rev: &str) -> Result<String, GitError> {
	let out = run(
		repo,
		Verb::RevParse,
		&["--verify", "--quiet", &format!("{rev}^{{commit}}")],
	)?;
	Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// Reads the checked-out branch's upstream remote and merge ref (branch.<name>.remote and .merge)
/// as git resolves them, the commit its saved tracking ref names, and the branch's own commit,
/// then checks that the branch contains the tracking commit. A detached HEAD, no upstream, a merge
/// ref outside refs/heads, a missing tracking ref or a branch behind it is an error, never a guess.
pub fn plan(repo: &Path) -> Result<PushPlan, GitError> {
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
	let seen = commit_of(repo, tracking)
		.map_err(|_| refused("This branch's upstream is gone from the remote"))?;
	// The branch ref itself, not HEAD again: the commit checked here is the one sent.
	let source = commit_of(repo, &head)?;
	let behind = run(
		repo,
		Verb::Log,
		&["-1", "--format=%H", &seen, &format!("^{source}"), "--"],
	)?;
	if !behind.is_empty() {
		return Err(refused(
			"This branch is behind its upstream; Pull first, or merge it in your own tools",
		));
	}
	Ok(PushPlan {
		remote: remote.to_string(),
		merge: merge.to_string(),
		seen,
		source,
	})
}
