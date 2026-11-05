//! Windows: Explorer for folders, and the local UTC offset read once from `w32tm /tz`.

use std::path::Path;
use std::process::Command;

use super::Platform;

pub struct Windows;

/// Explorer wants backslashes: given `C:/roots/x\repo` (a config root written with forward
/// slashes, joined by the scanner) it opens Documents instead of the folder, silently. Seen on
/// screen 2026-11-02 with a fixture root; every path handed to explorer.exe or the editor goes
/// through here. Git paths inside a repository are joined by `git_worktree_path`.
fn backslashed(path: &Path) -> String {
	path.display().to_string().replace('/', "\\")
}

impl Platform for Windows {
	fn open_folder(&self, dir: &Path) -> Result<(), String> {
		Command::new("explorer.exe")
			.arg(backslashed(dir))
			.spawn()
			.map(|_| ())
			.map_err(|e| e.to_string())
	}

	/// Explorer wants `/select,"C:\path with spaces\file"`, quotes around the path only, so the
	/// argument goes through raw and not through the usual quoting of the whole token. A deleted
	/// file has nothing to select; its folder opens instead.
	fn reveal_file(&self, file: &Path) -> Result<(), String> {
		use std::os::windows::process::CommandExt;
		if !file.exists() {
			return match file.parent() {
				Some(dir) => self.open_folder(dir),
				None => Err("no folder to open".to_string()),
			};
		}
		let select = format!("/select,\"{}\"", backslashed(file));
		Command::new("explorer.exe")
			.raw_arg(select)
			.spawn()
			.map(|_| ())
			.map_err(|e| e.to_string())
	}

	/// `<command> --goto <file>:<line>` for a file, `<command> <dir>` for a folder. VS Code's
	/// launcher on Windows is `code.cmd`; a bare `code` names the Unix shell script beside it,
	/// which cannot be started, so a command with no extension that is not found is retried as
	/// its `.cmd`, which std runs through cmd.exe with its own quoting. Nothing from a file's
	/// content reaches the command line.
	fn open_in_editor(
		&self,
		command: &str,
		target: &Path,
		line: Option<usize>,
	) -> Result<(), String> {
		// Through backslashed too, so a forward-slash path from a caller that skipped
		// git_worktree_path still reaches the editor in one form.
		let target = backslashed(target);
		let args: Vec<String> = match line {
			Some(n) => vec!["--goto".into(), format!("{target}:{n}")],
			None => vec![target],
		};
		start_detached(command, &args, Console::Hidden)
	}

	/// Windows Terminal opens a pwsh tab in the folder; without it pwsh itself opens there,
	/// through `shell_here`, since a console shell started by this windowless process directly
	/// gets no usable standard input and leaves at once (Morticia M-1 on 85ec3b0; the
	/// CREATE_NEW_CONSOLE attempt failed the same way under the ignored test).
	fn open_terminal(&self, command: &str, dir: &Path) -> Result<(), String> {
		let dir = dir.display().to_string();
		let via_terminal = [
			"-d".to_string(),
			dir.clone(),
			"pwsh".to_string(),
			"-NoLogo".to_string(),
		];
		match start_detached(command, &via_terminal, Console::New) {
			Ok(()) => Ok(()),
			Err(_) => shell_here(&dir).map_err(|e| format!("{command} not on PATH, and pwsh: {e}")),
		}
	}
}

/// Opens an interactive pwsh in `dir` by asking a hidden, non-interactive pwsh to Start-Process
/// it: Start-Process goes through the shell (ShellExecute), which gives the new console its own
/// standard handles whatever this process has. The hidden pwsh is waited for, so a failure to
/// start the shell is reported with its message rather than lost.
fn shell_here(dir: &str) -> Result<(), String> {
	start_shell(dir, &format!("repose: {dir}"))
}

/// The shell's window is titled, which also lets a test find the one it started and no other.
fn start_shell(dir: &str, title: &str) -> Result<(), String> {
	use std::process::Stdio;
	let quote = |s: &str| s.replace('\'', "''");
	let script = format!(
		"Start-Process -FilePath pwsh -WorkingDirectory '{}' -ArgumentList @('-NoLogo','-NoExit','-Command','$Host.UI.RawUI.WindowTitle = ''{}''')",
		quote(dir),
		quote(&quote(title))
	);
	let mut cmd = Command::new("pwsh");
	repose_core::git::commands::hide_console(&mut cmd);
	let out = cmd
		.args([
			"-NoProfile",
			"-NonInteractive",
			"-WindowStyle",
			"Hidden",
			"-Command",
		])
		.arg(&script)
		.stdin(Stdio::null())
		.output()
		.map_err(|e| e.to_string())?;
	if out.status.success() {
		Ok(())
	} else {
		let err = String::from_utf8_lossy(&out.stderr);
		Err(err
			.lines()
			.next()
			.unwrap_or("Start-Process failed")
			.to_string())
	}
}

/// What console a started program gets: none (an editor, which has its own window), or a new
/// one with real handles (a shell).
#[derive(Clone, Copy)]
enum Console {
	Hidden,
	New,
}

/// Starts a program with no pipes back to this process, hidden or in a new console, retrying a
/// bare name as its `.cmd` when it is not on PATH. The error names the command.
fn start_detached(command: &str, args: &[String], console: Console) -> Result<(), String> {
	use std::io::ErrorKind;
	let start = |program: &str| spawn_detached(program, args, console).map(|_| ());
	let not_found = || format!("{command} was not found on PATH");
	match start(command) {
		Ok(()) => Ok(()),
		Err(e) if e.kind() == ErrorKind::NotFound => {
			if Path::new(command).extension().is_some() {
				return Err(not_found());
			}
			start(&format!("{command}.cmd")).map_err(|_| not_found())
		}
		Err(e) => Err(e.to_string()),
	}
}

/// The spawn behind `start_detached`: hidden with every handle at NUL, or in a new console that
/// supplies the standard handles, with nothing redirected.
fn spawn_detached(
	program: &str,
	args: &[String],
	console: Console,
) -> std::io::Result<std::process::Child> {
	use std::os::windows::process::CommandExt;
	use std::process::Stdio;
	const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
	let mut cmd = Command::new(program);
	cmd.args(args);
	match console {
		Console::Hidden => {
			repose_core::git::commands::hide_console(&mut cmd);
			cmd.stdin(Stdio::null())
				.stdout(Stdio::null())
				.stderr(Stdio::null());
		}
		Console::New => {
			cmd.creation_flags(CREATE_NEW_CONSOLE);
		}
	}
	cmd.spawn()
}

/// Minutes east of UTC, re-read at most once a minute so a daylight change shows within the
/// minute. `std` has no local-time API, so this asks `w32tm /tz`, which prints the zone's bias and
/// daylight adjustment in minutes. When that fails the clock is shown in UTC and says so.
pub fn utc_offset_minutes() -> Option<i32> {
	use std::sync::Mutex;
	use std::time::{Duration, Instant};
	static CACHE: Mutex<Option<(Instant, Option<i32>)>> = Mutex::new(None);
	let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
	if let Some((at, offset)) = *cache
		&& at.elapsed() < Duration::from_secs(60)
	{
		return offset;
	}
	let mut cmd = Command::new("w32tm");
	repose_core::git::commands::hide_console(&mut cmd);
	let out = cmd.arg("/tz").output().ok();
	let text = out
		.map(|o| String::from_utf8_lossy(&o.stdout).to_string())
		.unwrap_or_default();
	let offset = parse_w32tm(&text);
	*cache = Some((Instant::now(), offset));
	offset
}

/// `Time zone: Current:TIME_ZONE_ID_STANDARD Bias: 360min (UTC=LocalTime+Bias)` and a
/// `[Standard Name:... Bias:0min ...]` and `[Daylight Name:... Bias:-60min ...]` pair.
fn parse_w32tm(text: &str) -> Option<i32> {
	let current_daylight = text.contains("TIME_ZONE_ID_DAYLIGHT");
	let mut biases = text.split("Bias:").skip(1).filter_map(|rest| {
		let num: String = rest
			.trim_start()
			.chars()
			.take_while(|c| c.is_ascii_digit() || *c == '-')
			.collect();
		num.parse::<i32>().ok()
	});
	let base = biases.next()?;
	let _standard = biases.next().unwrap_or(0);
	let daylight = biases.next().unwrap_or(0);
	let total = if current_daylight {
		base + daylight
	} else {
		base
	};
	Some(-total)
}

#[cfg(test)]
mod tests {
	use super::parse_w32tm;

	#[test]
	fn central_standard_time() {
		let text = "Time zone: Current:TIME_ZONE_ID_STANDARD Bias: 360min (UTC=LocalTime+Bias)\n  [Standard Name:\"Central Standard Time\" Bias:0min Date:(M:11 D:1 DoW:0)]\n  [Daylight Name:\"Central Daylight Time\" Bias:-60min Date:(M:3 D:2 DoW:0)]\n";
		assert_eq!(parse_w32tm(text), Some(-360));
	}

	#[test]
	fn central_daylight_time() {
		let text = "Time zone: Current:TIME_ZONE_ID_DAYLIGHT Bias: 360min (UTC=LocalTime+Bias)\n  [Standard Name:\"Central Standard Time\" Bias:0min Date:(M:11 D:1 DoW:0)]\n  [Daylight Name:\"Central Daylight Time\" Bias:-60min Date:(M:3 D:2 DoW:0)]\n";
		assert_eq!(parse_w32tm(text), Some(-300));
	}

	/// Morticia M-1 on 85ec3b0: pwsh with its standard input at NUL ignores -NoExit and exits at
	/// once, and so did one given CREATE_NEW_CONSOLE by this process (the first version of this
	/// test proved that). The fallback goes through Start-Process instead; this opens one shell
	/// in the temp folder, finds it by its command line after three seconds, and closes it.
	/// Ignored by default because it opens a window; run with --ignored in a slot and the result
	/// goes in the external-actions record.
	#[test]
	#[ignore]
	fn shell_here_opens_a_pwsh_that_stays() {
		// The marker is in the shell's own command line, so only the shell this test started is
		// found and closed; no other pwsh is touched. An apostrophe in both the folder and the
		// title runs the two levels of quoting through Start-Process (Roger #30994). What a pass
		// shows: Start-Process accepted both arguments, a shell with the marker stayed open, and
		// the folder could be removed after it was closed. What it does not show: the title text
		// in the window (a -NoExit shell survives a failed -Command; Morticia's separate run read
		// five titles back intact through the same doubling) or the window in the foreground.
		let stem = format!("repose-shell-test-{}", std::process::id());
		let dir = std::env::temp_dir().join(format!("{stem}'s"));
		std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("dir: {e}"));
		super::start_shell(&dir.display().to_string(), &format!("{stem}'s"))
			.unwrap_or_else(|e| panic!("start_shell: {e}"));
		std::thread::sleep(std::time::Duration::from_secs(3));
		let found = pwsh_processes_whose_command_line_contains(&stem);
		for pid in &found {
			let _ = std::process::Command::new("taskkill")
				.args(["/PID", &pid.to_string(), "/F"])
				.output();
		}
		// The shell held the folder as its working directory; once it is gone the folder goes.
		let mut removed = false;
		for _ in 0..20 {
			if std::fs::remove_dir(&dir).is_ok() || !dir.exists() {
				removed = true;
				break;
			}
			std::thread::sleep(std::time::Duration::from_millis(250));
		}
		assert!(removed, "the test folder {} was not removed", dir.display());
		assert!(
			!found.is_empty(),
			"no pwsh titled {stem}'s was running three seconds after Start-Process"
		);
	}

	fn pwsh_processes_whose_command_line_contains(marker: &str) -> Vec<u32> {
		let list = format!(
			"(Get-CimInstance Win32_Process -Filter \"Name='pwsh.exe'\" | Where-Object {{ $_.CommandLine -like '*{marker}*' -and $_.ProcessId -ne $PID }} | ForEach-Object ProcessId) -join ' '"
		);
		let out = std::process::Command::new("pwsh")
			.args(["-NoProfile", "-NonInteractive", "-Command", &list])
			.output()
			.unwrap_or_else(|e| panic!("pwsh: {e}"));
		String::from_utf8_lossy(&out.stdout)
			.split_whitespace()
			.filter_map(|s| s.parse().ok())
			.collect()
	}
}
