//! One commit as the History view shows it, plus the files it changed when expanded.

use super::ChangeKind;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Commit {
	pub hash: String,
	pub short: String,
	pub author: String,
	/// Author time as seconds since the Unix epoch, from git's ISO date.
	pub time: i64,
	/// The author date as git printed it (`%aI`), for the tooltip and the past-a-month fallback.
	pub date: String,
	pub subject: String,
	pub body: String,
	/// Parent hashes, first parent first (`%P`); empty for a root commit.
	pub parents: Vec<String>,
	/// The ref names that point at this commit (`%D`), such as `main`, `origin/main` or
	/// `tag: v1.0`; a `HEAD -> main` decoration is kept as `main`, and a bare `HEAD` is dropped.
	pub refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitFile {
	pub path: String,
	pub kind: ChangeKind,
	pub from: Option<String>,
}

/// `2026-11-01T04:36:21-06:00` to Unix seconds, without a date crate. Returns None for anything
/// that is not that exact shape.
pub fn parse_iso(text: &str) -> Option<i64> {
	let b = text.as_bytes();
	if b.len() < 19 {
		return None;
	}
	let num = |s: &str| s.parse::<i64>().ok();
	let year = num(text.get(0..4)?)?;
	let month = num(text.get(5..7)?)?;
	let day = num(text.get(8..10)?)?;
	let hour = num(text.get(11..13)?)?;
	let minute = num(text.get(14..16)?)?;
	let second = num(text.get(17..19)?)?;
	let offset = match text.get(19..) {
		Some("Z") | Some("") | None => 0,
		Some(rest) => {
			let sign = if rest.starts_with('-') { -1 } else { 1 };
			let oh = num(rest.get(1..3)?)?;
			let om = num(rest.get(4..6)?)?;
			sign * (oh * 3600 + om * 60)
		}
	};
	let days = days_from_civil(year, month, day);
	Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Howard Hinnant's days-from-civil, valid for the proleptic Gregorian calendar.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
	let y = if m <= 2 { y - 1 } else { y };
	let era = if y >= 0 { y } else { y - 399 } / 400;
	let yoe = y - era * 400;
	let mp = (m + 9) % 12;
	let doy = (153 * mp + 2) / 5 + d - 1;
	let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
	era * 146_097 + doe - 719_468
}

/// `3 min ago`, `2 h ago`, `5 d ago`, or the ISO date past a month.
pub fn relative(time: i64, now: i64, iso: &str) -> String {
	let delta = now - time;
	match delta {
		d if d < 60 => "just now".to_string(),
		d if d < 3_600 => format!("{} min ago", d / 60),
		d if d < 86_400 => format!("{} h ago", d / 3_600),
		d if d < 30 * 86_400 => format!("{} d ago", d / 86_400),
		_ => iso.get(0..10).unwrap_or(iso).to_string(),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn iso_with_offset() {
		// 2026-11-01T04:36:21-06:00 is 10:36:21Z.
		assert_eq!(parse_iso("2026-11-01T04:36:21-06:00"), Some(1_793_529_381));
		assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0));
		assert_eq!(parse_iso("2000-02-29T12:00:00+01:00"), Some(951_822_000));
		assert_eq!(parse_iso("nonsense"), None);
	}

	#[test]
	fn relative_buckets() {
		assert_eq!(relative(100, 130, ""), "just now");
		assert_eq!(relative(0, 7_200, ""), "2 h ago");
		assert_eq!(relative(0, 3 * 86_400, ""), "3 d ago");
		assert_eq!(
			relative(0, 40 * 86_400, "2026-11-01T04:36:21-06:00"),
			"2026-11-01"
		);
	}
}
