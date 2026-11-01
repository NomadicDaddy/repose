//! Windows: Explorer for folders, and the local UTC offset read once from `w32tm /tz`.

use std::path::Path;
use std::process::Command;

use super::Platform;

pub struct Windows;

impl Platform for Windows {
	fn open_folder(&self, dir: &Path) -> Result<(), String> {
		Command::new("explorer.exe")
			.arg(dir)
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
		let select = format!("/select,\"{}\"", file.display());
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
		let args: Vec<String> = match line {
			Some(n) => vec!["--goto".into(), format!("{}:{n}", target.display())],
			None => vec![target.display().to_string()],
		};
		start_detached(command, &args, Console::Hidden)
	}

	/// Windows Terminal opens a pwsh tab in the folder; without it pwsh itself opens there. Both
	/// get a new console of their own with its handles, because pwsh started with its standard
	/// input at NUL ignores -NoExit and leaves at once (Morticia M-1 on 85ec3b0).
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
			Err(_) => start_detached("pwsh", &pwsh_here(dir), Console::New)
				.map_err(|e| format!("{command} and pwsh: {e}")),
		}
	}
}

fn pwsh_here(dir: String) -> [String; 4] {
	[
		"-NoLogo".into(),
		"-NoExit".into(),
		"-WorkingDirectory".into(),
		dir,
	]
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
	/// once, so the fallback terminal must get a console of its own. Ignored by default because
	/// it opens a console window on the developer's screen; run with --ignored in a slot and the
	/// result goes in the external-actions record.
	#[test]
	#[ignore]
	fn pwsh_no_exit_stays_open_in_a_new_console() {
		let dir = std::env::temp_dir().display().to_string();
		let mut child = super::spawn_detached("pwsh", &super::pwsh_here(dir), super::Console::New)
			.unwrap_or_else(|e| panic!("pwsh did not start: {e}"));
		std::thread::sleep(std::time::Duration::from_millis(2500));
		let exited = child.try_wait().unwrap_or_else(|e| panic!("try_wait: {e}"));
		let _ = child.kill();
		let _ = child.wait();
		assert!(exited.is_none(), "pwsh -NoExit left at once: {exited:?}");
	}
}
