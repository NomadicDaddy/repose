//! Whether a GitHub repository is public or private (remote-visibility), read through the
//! installed `gh` on the user's Check visibility only (A1, Phillip #59749 on #59638). The one call
//! is `gh api --hostname github.com --method GET repos/<owner>/<repo> --jq .visibility`. It never
//! prompts or checks for updates, and repose never reads, stores or shows a token: gh uses its own
//! sign-in, as git's credential helpers are the user's own configuration.

use std::ffi::OsStr;
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::git::commands::hide_console;
use crate::git::remote::display_url;

/// How long gh may take before the check stops it and says so.
pub const TIMEOUT: Duration = Duration::from_secs(30);

/// The reason shown when the origin is not a github.com remote; nothing runs for it.
pub const NOT_GITHUB: &str =
	"Only github.com remotes in https or git@github.com: form can be checked";
/// The reason shown when `gh` cannot be found.
pub const NO_GH: &str = "GitHub CLI (gh) is not installed or not on PATH";
/// The reason shown when the owner or repository name is not one repose passes to gh.
pub const UNSAFE_NAME: &str =
	"The origin's owner or repository name has characters repose will not pass to gh";

/// What the host says, or why it could not be told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
	Public,
	Private,
	/// Not known, with the reason the header's tooltip shows.
	Unknown(String),
}

/// The owner and repository a github.com remote names: https (`https://[user@]github.com/o/r`)
/// or scp (`[user@]github.com:o/r`), with or without a trailing `.git` or `/`. A query or
/// fragment on an https URL is dropped with its userinfo (`display_url`). Each name must match
/// `[A-Za-z0-9._-]+`, must not start with `-` (gh would read it as a flag) and must not be all
/// dots (a path step). Anything else is the reason nothing will run.
pub fn repository(url: &str) -> Result<(String, String), &'static str> {
	let url = url.trim();
	let (host, path) = match url.split_once("://") {
		Some((scheme, _)) => {
			if !scheme.eq_ignore_ascii_case("https") {
				return Err(NOT_GITHUB);
			}
			let shown = display_url(url);
			let rest = shown.split_once("://").map_or("", |(_, rest)| rest);
			let (host, path) = rest.split_once('/').ok_or(NOT_GITHUB)?;
			(host.to_string(), path.to_string())
		}
		None => {
			let (user_host, path) = url.split_once(':').ok_or(NOT_GITHUB)?;
			let host = user_host.rsplit_once('@').map_or(user_host, |(_, h)| h);
			(host.to_string(), path.to_string())
		}
	};
	if !host.eq_ignore_ascii_case("github.com") {
		return Err(NOT_GITHUB);
	}
	let path = path.trim_end_matches('/');
	let path = path.strip_suffix(".git").unwrap_or(path);
	let (owner, repo) = path.split_once('/').ok_or(NOT_GITHUB)?;
	if repo.contains('/') {
		return Err(NOT_GITHUB);
	}
	if !safe_name(owner) || !safe_name(repo) {
		return Err(UNSAFE_NAME);
	}
	Ok((owner.to_string(), repo.to_string()))
}

fn safe_name(name: &str) -> bool {
	!name.is_empty()
		&& !name.starts_with('-')
		&& !name.bytes().all(|b| b == b'.')
		&& name
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// The one gh call, for names `repository` accepted: a GET of the repository's `visibility`
/// field. No prompt, no update check, a null stdin and no console window.
pub fn command(program: &OsStr, owner: &str, repo: &str) -> Command {
	let mut cmd = Command::new(program);
	hide_console(&mut cmd);
	cmd.args(["api", "--hostname", "github.com", "--method", "GET"])
		.arg(format!("repos/{owner}/{repo}"))
		.args(["--jq", ".visibility"])
		.env("GH_PROMPT_DISABLED", "1")
		.env("GH_NO_UPDATE_NOTIFIER", "1")
		.env("GH_NO_EXTENSION_UPDATE_NOTIFIER", "1")
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped());
	cmd
}

/// Asks the installed `gh` about `owner/repo`, names `repository` gave for the raw origin URL. The
/// caller validates the URL it was given, not a cleaned copy (Roger 5Q-3 on b551265); a name that
/// is not safe is refused here too, so nothing unchecked can reach gh.
pub fn ask(owner: &str, repo: &str) -> Visibility {
	ask_with(OsStr::new("gh"), owner, repo)
}

/// `repository` then `ask` through `program` in place of `gh`, so the tests can stand one in. A
/// URL that is not a github.com remote is Unknown with nothing run.
pub fn check_with(program: &OsStr, url: &str) -> Visibility {
	match repository(url) {
		Ok((owner, repo)) => ask_with(program, &owner, &repo),
		Err(why) => Visibility::Unknown(why.to_string()),
	}
}

/// `ask` through `program` in place of `gh`.
pub fn ask_with(program: &OsStr, owner: &str, repo: &str) -> Visibility {
	if !safe_name(owner) || !safe_name(repo) {
		return Visibility::Unknown(UNSAFE_NAME.to_string());
	}
	let mut child = match command(program, owner, repo).spawn() {
		Ok(child) => child,
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
			return Visibility::Unknown(NO_GH.to_string());
		}
		Err(e) => return Visibility::Unknown(format!("gh could not start: {e}")),
	};
	let stdout = drain(child.stdout.take());
	let stderr = drain(child.stderr.take());
	match wait(&mut child) {
		Ok(status) => {
			let wait_pipe = |rx: mpsc::Receiver<Vec<u8>>| {
				rx.recv_timeout(Duration::from_secs(5)).unwrap_or_default()
			};
			answer(status.code(), &wait_pipe(stdout), &wait_pipe(stderr))
		}
		Err(why) => Visibility::Unknown(why),
	}
}

/// Waits for gh up to `TIMEOUT`, stopping it after that.
fn wait(child: &mut Child) -> Result<ExitStatus, String> {
	let started = Instant::now();
	loop {
		match child.try_wait() {
			Ok(Some(status)) => return Ok(status),
			Ok(None) if started.elapsed() > TIMEOUT => {
				let _ = child.kill();
				let _ = child.wait();
				return Err(format!("gh did not answer within {} s", TIMEOUT.as_secs()));
			}
			Ok(None) => std::thread::sleep(Duration::from_millis(20)),
			Err(e) => return Err(format!("gh could not be waited on: {e}")),
		}
	}
}

/// Reads one pipe to its end on a thread, so gh never blocks on a full pipe.
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

/// What gh's exit code (None when it was stopped) and output mean. Success prints `public` or
/// `private`; GitHub's third value, `internal`, and anything else is Unknown with what gh said. A
/// failure (signed out, a 404, no network) is Unknown with gh's last line of error text.
pub fn answer(exit: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Visibility {
	if exit == Some(0) {
		let said = String::from_utf8_lossy(stdout).trim().to_string();
		return match said.as_str() {
			"public" => Visibility::Public,
			"private" => Visibility::Private,
			"internal" => Visibility::Unknown(
				"GitHub says internal: visible to its enterprise's members".to_string(),
			),
			_ => Visibility::Unknown(format!(
				"gh answered \"{}\", which is neither public nor private",
				said.chars().take(60).collect::<String>()
			)),
		};
	}
	let text = String::from_utf8_lossy(stderr);
	let last = text.lines().map(str::trim).rfind(|l| !l.is_empty());
	Visibility::Unknown(match last {
		Some(line) => {
			let line = line.strip_prefix("gh: ").unwrap_or(line);
			format!("gh: {}", line.chars().take(200).collect::<String>())
		}
		None => match exit {
			Some(code) => format!("gh failed with exit code {code} and no message"),
			None => "gh stopped with no message".to_string(),
		},
	})
}
