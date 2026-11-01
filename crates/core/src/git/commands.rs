//! The one place that spawns `git`: an allowed-verb list, the environment every spawn gets, a
//! bounded pool of eight processes, and a timeout on reads only.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Condvar, Mutex, OnceLock, mpsc};
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
	/// `git rm --cached`: the only way to unstage on a repository with no commit yet.
	RmCached,
	RemoteGetUrl,
}

impl Verb {
	pub const ALL: [Verb; 13] = [
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
		Verb::RmCached,
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
			// --force with --cached removes only the index entry; the working file is never
			// touched, and without it an edited staged file blocks the whole group.
			Verb::RmCached => &["rm", "--cached", "--force", "--quiet"],
			Verb::RemoteGetUrl => &["remote", "get-url"],
		}
	}

	/// Writes have no timeout, because a hooked commit can run for minutes.
	pub fn is_write(self) -> bool {
		matches!(
			self,
			Verb::Add | Verb::Restore | Verb::Clean | Verb::Commit | Verb::RmCached
		)
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
	/// git exited but its output never arrived in time, usually because a process it started is
	/// still holding the pipe. A read must not be mistaken for an empty result.
	OutputLost,
	/// git reported success but the paths it was asked to remove are still there, as `git clean`
	/// does for a nested repository. Success must not be claimed for them.
	StillPresent(Vec<String>),
}

impl std::fmt::Display for GitError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::StillPresent(paths) => write!(
				f,
				"git clean left {} in place (a nested repository is not removed): {}",
				if paths.len() == 1 {
					"this path"
				} else {
					"these paths"
				},
				paths.join(", ")
			),
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
			Self::OutputLost => write!(f, "git exited but its output was not collected"),
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

/// A windowed app has no console, so without this flag every child `git.exe` opens its own
/// terminal window. CREATE_NO_WINDOW keeps the children invisible; pipes still work.
#[cfg(windows)]
pub fn hide_console(cmd: &mut Command) {
	use std::os::windows::process::CommandExt;
	const CREATE_NO_WINDOW: u32 = 0x0800_0000;
	cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn hide_console(_cmd: &mut Command) {}

/// Builds the command every spawn shares: literal pathspecs always; no optional locks and no
/// fsmonitor program on reads; stdin closed; no console window; the repository as working
/// directory.
pub fn command(repo: &Path, verb: Verb, extra: &[&str]) -> Command {
	let mut cmd = Command::new("git");
	hide_console(&mut cmd);
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
		// No signature text in parsed log output on a machine that sets log.showSignature.
		cmd.env("GIT_OPTIONAL_LOCKS", "0")
			.arg("-c")
			.arg("core.fsmonitor=false")
			.arg("-c")
			.arg("log.showSignature=false");
	}
	cmd.args(verb.args()).args(extra);
	cmd
}

/// Drains one pipe on its own thread from the moment of spawn, so output larger than the pipe's
/// buffer never blocks git. The bytes arrive on the channel when the pipe closes.
fn drain<R: std::io::Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<Vec<u8>> {
	let (tx, rx) = mpsc::channel();
	std::thread::spawn(move || {
		let mut buf = Vec::new();
		if let Some(mut pipe) = pipe {
			let _ = pipe.read_to_end(&mut buf);
		}
		let _ = tx.send(buf);
	});
	rx
}

/// Collects a drained pipe, giving up after `wait` if a grandchild still holds the write end.
/// None means the bytes never arrived; callers decide whether that is an error.
fn collect(rx: &mpsc::Receiver<Vec<u8>>, wait: Duration) -> Option<Vec<u8>> {
	rx.recv_timeout(wait).ok()
}

/// Runs a verb and returns stdout bytes. Reads time out after `READ_TIMEOUT`; writes never do.
/// Both pipes are read concurrently from spawn, so large output cannot deadlock the child.
pub fn run(repo: &Path, verb: Verb, extra: &[&str]) -> Result<Vec<u8>, GitError> {
	let _permit = Permit::acquire();
	let mut child = command(repo, verb, extra).spawn().map_err(|e| {
		if e.kind() == std::io::ErrorKind::NotFound {
			GitError::NotFound
		} else {
			GitError::Spawn(e.to_string())
		}
	})?;
	let stdout = drain(child.stdout.take());
	let stderr = drain(child.stderr.take());
	let started = Instant::now();
	let timeout = (!verb.is_write()).then_some(READ_TIMEOUT);
	let status = loop {
		match child.try_wait() {
			Ok(Some(status)) => break status,
			Ok(None) => {
				if let Some(limit) = timeout.filter(|l| started.elapsed() > *l) {
					let _ = child.kill();
					let _ = child.wait();
					let err = collect(&stderr, Duration::from_millis(500)).unwrap_or_default();
					return Err(GitError::TimedOut {
						after: limit,
						stderr_tail: tail(&err),
					});
				}
				std::thread::sleep(Duration::from_millis(5));
			}
			Err(e) => return Err(GitError::Spawn(e.to_string())),
		}
	};
	let out = collect(&stdout, Duration::from_secs(5));
	let err = collect(&stderr, Duration::from_secs(5)).unwrap_or_default();
	if status.success() {
		// A read whose bytes never arrived is an error, not an empty result: a dirty repository
		// must never look clean because something held the pipe open.
		match out {
			Some(bytes) => Ok(bytes),
			None if verb.is_write() => Ok(Vec::new()),
			None => Err(GitError::OutputLost),
		}
	} else {
		Err(GitError::Failed {
			code: status.code(),
			stderr_tail: tail(&err),
		})
	}
}

fn tail(bytes: &[u8]) -> String {
	let text = String::from_utf8_lossy(bytes);
	let lines: Vec<&str> = text.lines().rev().take(3).collect();
	lines.into_iter().rev().collect::<Vec<_>>().join(" | ")
}
