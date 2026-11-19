//! A suggested commit message for the staged changes (ai-commit-message-suggestions), asked of the
//! installed Claude Code CLI on the user's Suggest message only (A1, Phillip #59782 on #59780).
//! This is the only module that spawns it, as `git::commands` is the only one that spawns git and
//! `host::github` the only one that spawns gh.
//!
//! The request is a fixed instruction plus the staged diff, on stdin. The CLI runs in an empty
//! temporary directory with no tools, no MCP servers, no hooks, plugins or CLAUDE.md, and no saved
//! session, so it is given nothing but the request and can act on nothing. Its sign-in and its
//! call to its provider are its own; repose never reads, stores or shows a key or token.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::git::Cancel;
use crate::git::commands::hide_console;
use crate::git::kill::kill_tree;

/// The most staged diff a request carries (Phillip #59774).
pub const CAP: usize = 64 * 1024;
/// How long the CLI may take before the suggestion stops it and says so.
pub const TIMEOUT: Duration = Duration::from_secs(120);
/// The reason shown when `claude` cannot be found.
pub const NO_CLI: &str = "Claude Code (claude) is not installed or not on PATH";
/// The reason shown when nothing is staged.
pub const NOTHING_STAGED: &str = "Nothing is staged to describe";
/// The reason shown after the user's Cancel.
pub const CANCELLED: &str = "Suggestion cancelled";

/// What the model is asked for; the staged diff follows it.
pub const INSTRUCTION: &str = "Write a git commit message for the staged changes below. Reply \
with the message only: a subject line of at most 72 characters in the imperative mood, then, \
only if it helps, a blank line and a short body wrapped at 72 characters. No code fences, no \
quotes and no commentary.";

/// The arguments Claude Code runs with, pinned by A1's Check: print mode with text output, no
/// built-in tools, safe mode (no hooks, MCP servers, plugins, skills or CLAUDE.md), no MCP server
/// from any config, no slash commands and no saved session.
pub const ARGS: [&str; 9] = [
	"--print",
	"--output-format",
	"text",
	"--tools",
	"",
	"--safe-mode",
	"--strict-mcp-config",
	"--disable-slash-commands",
	"--no-session-persistence",
];

/// The text sent on stdin, and the staged files whose diffs did not fit under `CAP`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
	pub text: String,
	pub left_out: Vec<String>,
}

/// A suggested message, and the staged files whose diffs it was written without.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
	pub message: String,
	pub left_out: Vec<String>,
}

/// Builds the request from `git diff --cached` output. Every staged path is listed. Whole-file
/// diffs follow in git's order until the next one would take the diffs past `CAP`; a file's diff
/// is never cut part-way, and the paths left out are named in the request and returned.
pub fn request(diff: &[u8]) -> Request {
	let files = split(diff);
	let mut kept = String::new();
	let mut left_out = Vec::new();
	for (path, body) in &files {
		if left_out.is_empty() && kept.len() + body.len() <= CAP {
			kept.push_str(body);
		} else {
			left_out.push(path.clone());
		}
	}
	let mut text = format!("{INSTRUCTION}\n\nStaged files:\n");
	for (path, _) in &files {
		text.push_str(&format!("- {path}\n"));
	}
	if !left_out.is_empty() {
		text.push_str(&format!(
			"\nThe diffs of {} of these files are left out to stay under {} KiB: {}.\n",
			left_out.len(),
			CAP / 1024,
			left_out.join(", ")
		));
	}
	text.push_str("\nDiff:\n");
	text.push_str(&kept);
	Request { text, left_out }
}

/// The diff cut at each `diff --git ` line, named by the path after ` b/`.
fn split(diff: &[u8]) -> Vec<(String, String)> {
	let text = String::from_utf8_lossy(diff);
	let mut files: Vec<(String, String)> = Vec::new();
	for line in text.split_inclusive('\n') {
		if let Some(header) = line.strip_prefix("diff --git ") {
			let path = header
				.rsplit_once(" b/")
				.map_or(header, |(_, b)| b)
				.trim_end()
				.to_string();
			files.push((path, String::new()));
		}
		if let Some((_, body)) = files.last_mut() {
			body.push_str(line);
		}
	}
	files
}

/// The one Claude Code call, run in `dir`: the pinned `ARGS`, a piped stdin the request is written
/// to and then closed, and no console window.
pub fn command(program: &OsStr, dir: &Path) -> Command {
	let mut cmd = Command::new(program);
	hide_console(&mut cmd);
	cmd.args(ARGS)
		.current_dir(dir)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped());
	cmd
}

/// Asks the installed `claude` for a message for `diff`, the staged changes.
pub fn suggest(diff: &[u8], cancel: &Cancel) -> Result<Suggestion, String> {
	suggest_with(OsStr::new("claude"), diff, cancel)
}

/// `suggest` through `program` in place of `claude`, so the tests can stand one in.
pub fn suggest_with(program: &OsStr, diff: &[u8], cancel: &Cancel) -> Result<Suggestion, String> {
	if diff.iter().all(u8::is_ascii_whitespace) {
		return Err(NOTHING_STAGED.to_string());
	}
	let Request { text, left_out } = request(diff);
	let dir = EmptyDir::new().map_err(|e| format!("No empty folder for the request: {e}"))?;
	let mut child = match command(program, &dir.0).spawn() {
		Ok(child) => child,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(NO_CLI.to_string()),
		Err(e) => return Err(format!("Claude Code could not start: {e}")),
	};
	let stdout = drain(child.stdout.take());
	let stderr = drain(child.stderr.take());
	let writer = child.stdin.take().map(|mut stdin| {
		let text = text.into_bytes();
		std::thread::spawn(move || {
			let _ = stdin.write_all(&text);
		})
	});
	let exit = wait(&mut child, cancel);
	if let Some(writer) = writer {
		let _ = writer.join();
	}
	let exit = exit?;
	let pipe =
		|rx: mpsc::Receiver<Vec<u8>>| rx.recv_timeout(Duration::from_secs(5)).unwrap_or_default();
	let message = answer(exit, &pipe(stdout), &pipe(stderr))?;
	Ok(Suggestion { message, left_out })
}

/// Waits up to `TIMEOUT`, stopping the CLI and everything it started on a timeout or Cancel.
fn wait(child: &mut Child, cancel: &Cancel) -> Result<Option<i32>, String> {
	let started = Instant::now();
	loop {
		match child.try_wait() {
			Ok(Some(status)) => return Ok(status.code()),
			Ok(None) if cancel.is_cancelled() => {
				kill_tree(child);
				return Err(CANCELLED.to_string());
			}
			Ok(None) if started.elapsed() > TIMEOUT => {
				kill_tree(child);
				return Err(format!(
					"Claude Code did not answer within {} s",
					TIMEOUT.as_secs()
				));
			}
			Ok(None) => std::thread::sleep(Duration::from_millis(20)),
			Err(e) => return Err(format!("Claude Code could not be waited on: {e}")),
		}
	}
}

/// Reads one pipe to its end on a thread, so the CLI never blocks on a full pipe.
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<Vec<u8>> {
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

/// What the exit code and output mean: on success the message, with surrounding blank lines and
/// a code fence the model added anyway taken off; otherwise the CLI's last line of error text.
pub fn answer(exit: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<String, String> {
	if exit == Some(0) {
		let said = String::from_utf8_lossy(stdout);
		let message = unfence(said.trim()).trim().to_string();
		return if message.is_empty() {
			Err("Claude Code answered with an empty message".to_string())
		} else {
			Ok(message)
		};
	}
	let text = String::from_utf8_lossy(stderr);
	let last = text.lines().map(str::trim).rfind(|l| !l.is_empty());
	Err(match (last, exit) {
		(Some(line), _) => format!(
			"Claude Code: {}",
			line.chars().take(200).collect::<String>()
		),
		(None, Some(code)) => format!("Claude Code failed with exit code {code} and no message"),
		(None, None) => "Claude Code stopped with no message".to_string(),
	})
}

/// `text` without one enclosing ``` fence, if it has one.
fn unfence(text: &str) -> &str {
	let Some(rest) = text.strip_prefix("```") else {
		return text;
	};
	let Some(body) = rest.strip_suffix("```") else {
		return text;
	};
	body.split_once('\n').map_or(body, |(_, b)| b)
}

/// A new empty folder under the system temp folder, removed when the request is done.
struct EmptyDir(PathBuf);

impl EmptyDir {
	fn new() -> std::io::Result<Self> {
		static NEXT: AtomicU64 = AtomicU64::new(0);
		let n = NEXT.fetch_add(1, Ordering::SeqCst);
		let dir = std::env::temp_dir().join(format!("repose-suggest-{}-{n}", std::process::id()));
		std::fs::create_dir(&dir)?;
		Ok(Self(dir))
	}
}

impl Drop for EmptyDir {
	fn drop(&mut self) {
		let _ = std::fs::remove_dir_all(&self.0);
	}
}
