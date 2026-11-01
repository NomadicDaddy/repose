//! The one place that spawns `git`: an allowed-verb list, the environment every spawn gets, a
//! bounded pool of eight processes, and a timeout on reads only.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// The verbs repose may run. No fetch, pull, push, clone or ls-remote exists here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
	Status,
	RevParse,
	Log,
	Show,
	Diff,
	LsTree,
	CatFile,
	Add,
	Restore,
	Clean,
	Commit,
	RemoteGetUrl,
}

impl Verb {
	pub const ALL: [Verb; 12] = [
		Verb::Status,
		Verb::RevParse,
		Verb::Log,
		Verb::Show,
		Verb::Diff,
		Verb::LsTree,
		Verb::CatFile,
		Verb::Add,
		Verb::Restore,
		Verb::Clean,
		Verb::Commit,
		Verb::RemoteGetUrl,
	];

	pub fn args(self) -> &'static [&'static str] {
		match self {
			Verb::Status => &["status"],
			Verb::RevParse => &["rev-parse"],
			Verb::Log => &["log"],
			Verb::Show => &["show"],
			Verb::Diff => &["diff"],
			Verb::LsTree => &["ls-tree"],
			Verb::CatFile => &["cat-file"],
			Verb::Add => &["add"],
			Verb::Restore => &["restore"],
			Verb::Clean => &["clean"],
			Verb::Commit => &["commit"],
			Verb::RemoteGetUrl => &["remote", "get-url"],
		}
	}

	/// Writes have no timeout, because a hooked commit can run for minutes.
	pub fn is_write(self) -> bool {
		matches!(self, Verb::Add | Verb::Restore | Verb::Clean | Verb::Commit)
	}
}

#[derive(Debug, Clone)]
pub enum GitError {
	NotFound,
	Spawn(String),
	TimedOut {
		after: Duration,
		stderr_tail: String,
	},
	Failed {
		code: Option<i32>,
		stderr_tail: String,
	},
}

impl std::fmt::Display for GitError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::NotFound => write!(f, "git was not found on PATH"),
			Self::Spawn(e) => write!(f, "git could not start: {e}"),
			Self::TimedOut { after, stderr_tail } => {
				write!(f, "git timed out after {}s: {stderr_tail}", after.as_secs())
			}
			Self::Failed { code, stderr_tail } => {
				write!(
					f,
					"git exited {}: {stderr_tail}",
					code.map_or("?".into(), |c| c.to_string())
				)
			}
		}
	}
}

pub const READ_TIMEOUT: Duration = Duration::from_secs(10);
pub const POOL_SIZE: usize = 8;

struct Pool {
	free: Mutex<usize>,
	cv: Condvar,
}

fn pool() -> &'static Pool {
	static POOL: OnceLock<Pool> = OnceLock::new();
	POOL.get_or_init(|| Pool {
		free: Mutex::new(POOL_SIZE),
		cv: Condvar::new(),
	})
}

struct Permit;

impl Permit {
	fn acquire() -> Permit {
		let p = pool();
		let mut free = p.free.lock().unwrap_or_else(|e| e.into_inner());
		while *free == 0 {
			free = p.cv.wait(free).unwrap_or_else(|e| e.into_inner());
		}
		*free -= 1;
		Permit
	}
}

impl Drop for Permit {
	fn drop(&mut self) {
		let p = pool();
		let mut free = p.free.lock().unwrap_or_else(|e| e.into_inner());
		*free += 1;
		p.cv.notify_one();
	}
}

/// Builds the command every spawn shares: literal pathspecs always; no optional locks and no
/// fsmonitor program on reads; stdin closed; the repository as working directory.
pub fn command(repo: &Path, verb: Verb, extra: &[&str]) -> Command {
	let mut cmd = Command::new("git");
	cmd.current_dir(repo)
		.env("GIT_TERMINAL_PROMPT", "0")
		.env("GIT_LITERAL_PATHSPECS", "1")
		.env("LC_ALL", "C")
		.arg("-c")
		.arg("core.quotePath=false")
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped());
	if !verb.is_write() {
		cmd.env("GIT_OPTIONAL_LOCKS", "0")
			.arg("-c")
			.arg("core.fsmonitor=false");
	}
	cmd.args(verb.args()).args(extra);
	cmd
}

/// Runs a verb and returns stdout bytes. Reads time out after `READ_TIMEOUT`; writes never do.
pub fn run(repo: &Path, verb: Verb, extra: &[&str]) -> Result<Vec<u8>, GitError> {
	let _permit = Permit::acquire();
	let mut child = command(repo, verb, extra).spawn().map_err(|e| {
		if e.kind() == std::io::ErrorKind::NotFound {
			GitError::NotFound
		} else {
			GitError::Spawn(e.to_string())
		}
	})?;
	let started = Instant::now();
	let timeout = (!verb.is_write()).then_some(READ_TIMEOUT);
	loop {
		match child.try_wait() {
			Ok(Some(_)) => break,
			Ok(None) => {
				if let Some(limit) = timeout.filter(|l| started.elapsed() > *l) {
					let _ = child.kill();
					let out = child
						.wait_with_output()
						.map_err(|e| GitError::Spawn(e.to_string()))?;
					return Err(GitError::TimedOut {
						after: limit,
						stderr_tail: tail(&out.stderr),
					});
				}
				std::thread::sleep(Duration::from_millis(5));
			}
			Err(e) => return Err(GitError::Spawn(e.to_string())),
		}
	}
	let out = child
		.wait_with_output()
		.map_err(|e| GitError::Spawn(e.to_string()))?;
	if out.status.success() {
		Ok(out.stdout)
	} else {
		Err(GitError::Failed {
			code: out.status.code(),
			stderr_tail: tail(&out.stderr),
		})
	}
}

fn tail(bytes: &[u8]) -> String {
	let text = String::from_utf8_lossy(bytes);
	let lines: Vec<&str> = text.lines().rev().take(3).collect();
	lines.into_iter().rev().collect::<Vec<_>>().join(" | ")
}
