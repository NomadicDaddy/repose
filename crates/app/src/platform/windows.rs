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

	/// `<command> --goto <file>:<line>`. VS Code's launcher on Windows is `code.cmd`; a bare
	/// `code` names the Unix shell script beside it, which cannot be started, so a command with
	/// no extension that is not found is retried as its `.cmd`, which std runs through cmd.exe
	/// with its own quoting. Nothing from the file's content reaches the command line.
	fn open_in_editor(&self, command: &str, file: &Path, line: usize) -> Result<(), String> {
		use std::io::ErrorKind;
		use std::process::Stdio;
		let target = format!("{}:{line}", file.display());
		let start = |program: &str| {
			let mut cmd = Command::new(program);
			repose_core::git::commands::hide_console(&mut cmd);
			cmd.arg("--goto")
				.arg(&target)
				.stdin(Stdio::null())
				.stdout(Stdio::null())
				.stderr(Stdio::null())
				.spawn()
				.map(|_| ())
		};
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
}
