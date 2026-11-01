//! The one place that spawns `git`: an allowed-verb list, the environment every spawn gets, a
//! bounded pool of eight processes, and a timeout on reads only.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
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
	/// `git symbolic-ref`: which branch HEAD names, to tell an unborn branch from a broken one.
	SymbolicRef,
	RemoteGetUrl,
}

impl Verb {
	pub const ALL: [Verb; 14] = [
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
		Verb::SymbolicRef,
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
			Verb::SymbolicRef => &["symbolic-ref", "--quiet"],
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

pub use super::error::GitError;

/// A handle the user's Cancel sets; a write that was given it stops polling, kills git and the
/// whole tree git started, and returns `GitError::Cancelled`.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn cancel(&self) {
		self.0.store(true, Ordering::SeqCst);
	}

	pub fn is_cancelled(&self) -> bool {
		self.0.load(Ordering::SeqCst)
	}
}

pub const READ_TIMEOUT: Duration = Duration::from_secs(10);
pub use super::pool::POOL_SIZE;
use super::pool::Permit;

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
	run_cancellable(repo, verb, extra, None)
}

/// `run` with a Cancel: when it is set while git runs, git and every process it started (a
/// hook and the hook's children) are killed and the result is `GitError::Cancelled`.
pub fn run_cancellable(
	repo: &Path,
	verb: Verb,
	extra: &[&str],
	cancel: Option<&Cancel>,
) -> Result<Vec<u8>, GitError> {
	let _permit = Permit::acquire();
	// A Cancel clicked while this request waited for the pool must not start git at all (Roger
	// 5Q-1 on a31e63b); nothing ran, so nothing needs killing.
	if cancel.is_some_and(Cancel::is_cancelled) {
		return Err(GitError::Cancelled {
			stderr_tail: String::new(),
			tree_killed: true,
		});
	}
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
				if cancel.is_some_and(Cancel::is_cancelled) {
					let tree_killed = kill_tree(&mut child);
					let err = collect(&stderr, Duration::from_millis(500)).unwrap_or_default();
					return Err(GitError::Cancelled {
						stderr_tail: tail(&err),
						tree_killed,
					});
				}
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
		// A read's failure is a line of context; a write's is the hook's own output, which the
		// user has to act on, so a commit keeps the last 40 lines as lines (stage-and-commit:
		// hook output beyond a three-line tail).
		let stderr_tail = if verb.is_write() {
			tail_lines(&err, 40, "\n")
		} else {
			tail(&err)
		};
		Err(GitError::Failed {
			code: status.code(),
			stderr_tail,
		})
	}
}

/// Kills git and everything under it. Killing git alone would leave a hook and the hook's
/// children running on; on Windows `taskkill /T` takes the tree, and the child is reaped after.
/// Returns whether the tree kill itself succeeded; git is reaped either way, and a false answer
/// is reported to the user rather than claimed as stopped (Roger 5Q-2 on a31e63b).
fn kill_tree(child: &mut std::process::Child) -> bool {
	#[cfg(windows)]
	let tree_killed = {
		let mut kill = Command::new("taskkill");
		hide_console(&mut kill);
		kill.args(["/PID", &child.id().to_string(), "/T", "/F"])
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status()
			.is_ok_and(|s| s.success())
	};
	#[cfg(not(windows))]
	let tree_killed = child.kill().is_ok();
	let _ = child.kill();
	let _ = child.wait();
	tree_killed
}

fn tail(bytes: &[u8]) -> String {
	tail_lines(bytes, 3, " | ")
}

/// The most a kept tail may hold, whatever its line count: one enormous line from a hook is
/// cut to its last 8 KB rather than copied whole into the error and the window (Roger 5Q-2 on
/// 9a05e06).
const TAIL_BYTES: usize = 8 * 1024;

/// The last `n` lines of git's stderr, joined by `sep`, in order, and at most `TAIL_BYTES`.
fn tail_lines(bytes: &[u8], n: usize, sep: &str) -> String {
	let text = String::from_utf8_lossy(bytes);
	let lines: Vec<&str> = text.lines().rev().take(n).collect();
	let joined = lines.into_iter().rev().collect::<Vec<_>>().join(sep);
	if joined.len() <= TAIL_BYTES {
		return joined;
	}
	let mut start = joined.len() - TAIL_BYTES;
	while !joined.is_char_boundary(start) {
		start += 1;
	}
	format!("…{}", &joined[start..])
}
