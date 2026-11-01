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
}

/// Minutes east of UTC, read once. `std` has no local-time API, so this asks `w32tm /tz`, which
/// prints the zone's bias and daylight adjustment in minutes.
pub fn utc_offset_minutes() -> i32 {
	use std::sync::OnceLock;
	static OFFSET: OnceLock<i32> = OnceLock::new();
	*OFFSET.get_or_init(|| {
		let out = Command::new("w32tm").arg("/tz").output().ok();
		let text = out
			.map(|o| String::from_utf8_lossy(&o.stdout).to_string())
			.unwrap_or_default();
		parse_w32tm(&text).unwrap_or(0)
	})
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
