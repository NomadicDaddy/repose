//! The machine's local zone as `w32tm /tz` reports it: minutes east of UTC and the zone's short
//! name, so a time can be shown as Summon's footer shows one, `2026-11-14 23:36:58 CST`.

/// Minutes east of UTC, and the current zone's initials when its name could be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zone {
	pub offset_min: i32,
	pub abbrev: Option<String>,
}

/// `Time zone: Current:TIME_ZONE_ID_STANDARD Bias: 360min (UTC=LocalTime+Bias)` and a
/// `[Standard Name:... Bias:0min ...]` and `[Daylight Name:... Bias:-60min ...]` pair. Only the
/// Windows platform reads it; elsewhere the zone is never known.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_w32tm(text: &str) -> Option<Zone> {
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
	let label = if current_daylight {
		"[Daylight Name:"
	} else {
		"[Standard Name:"
	};
	Some(Zone {
		offset_min: -total,
		abbrev: zone_name(text, label).and_then(|name| initials(&name)),
	})
}

/// The quoted name after `label`, such as `Central Standard Time`.
fn zone_name(text: &str, label: &str) -> Option<String> {
	let rest = text.split(label).nth(1)?.trim_start().strip_prefix('"')?;
	Some(rest.split('"').next()?.to_string())
}

/// Windows names a zone in words; its short form is their initials, `Central Standard Time` to
/// `CST`. A name with no lettered word gives None, and the caller shows the numeric offset.
fn initials(name: &str) -> Option<String> {
	let short: String = name
		.split_whitespace()
		.filter_map(|word| word.chars().find(char::is_ascii_alphabetic))
		.map(|c| c.to_ascii_uppercase())
		.collect();
	(!short.is_empty()).then_some(short)
}

#[cfg(test)]
mod tests {
	use super::{Zone, parse_w32tm};

	const CENTRAL: &str = "  [Standard Name:\"Central Standard Time\" Bias:0min Date:(M:11 D:1 DoW:0)]\n  [Daylight Name:\"Central Daylight Time\" Bias:-60min Date:(M:3 D:2 DoW:0)]\n";

	fn zone(offset_min: i32, abbrev: &str) -> Option<Zone> {
		Some(Zone {
			offset_min,
			abbrev: Some(abbrev.to_string()),
		})
	}

	#[test]
	fn central_standard_time() {
		let text = format!(
			"Time zone: Current:TIME_ZONE_ID_STANDARD Bias: 360min (UTC=LocalTime+Bias)\n{CENTRAL}"
		);
		assert_eq!(parse_w32tm(&text), zone(-360, "CST"));
	}

	#[test]
	fn central_daylight_time() {
		let text = format!(
			"Time zone: Current:TIME_ZONE_ID_DAYLIGHT Bias: 360min (UTC=LocalTime+Bias)\n{CENTRAL}"
		);
		assert_eq!(parse_w32tm(&text), zone(-300, "CDT"));
	}

	#[test]
	fn a_missing_name_keeps_the_offset() {
		let text = "Time zone: Current:TIME_ZONE_ID_STANDARD Bias: -60min (UTC=LocalTime+Bias)\n";
		let read = parse_w32tm(text);
		assert_eq!(read.as_ref().map(|z| z.offset_min), Some(60));
		assert_eq!(read.and_then(|z| z.abbrev), None);
	}

	#[test]
	fn unreadable_output_is_none() {
		assert_eq!(parse_w32tm(""), None);
	}
}
