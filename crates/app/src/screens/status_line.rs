//! The repository list's status line: how many repositories, when they were last checked, and
//! what their counts are read from (saved refs, and how many this session fetched).

use super::fetch::list_note;
use crate::state::AppState;

/// The header status line; with `clock_shown` false, no "Checked HH:MM" (the live region).
/// `matched` is how many repositories the typed filter leaves, or None with no filter; it reads
/// "1 of 4 match" in place of the plain count (design-polish clause 4).
pub(super) fn status_line(
	state: &AppState,
	count: usize,
	matched: Option<usize>,
	clock_shown: bool,
) -> String {
	if state.scanning && state.checked_at.is_none() {
		return "Scanning…".to_string();
	}
	let time = state
		.checked_at
		.map(|t| clock(t, state.utc_offset))
		.unwrap_or_default();
	let lead = match matched {
		Some(n) => format!("{n} of {count} match"),
		None => super::count_label(count, "repository", "repositories"),
	};
	let missing = match state.missing_roots.len() {
		0 => String::new(),
		1 => format!(" · 1 root not found: {}", state.missing_roots[0].display()),
		n => format!(" · {n} roots not found"),
	};
	// The watcher's notes: a root it could not watch or no watcher at all (lasting), and Watcher
	// caught up after an overflow (until the next full scan completes).
	let mut note = state
		.watching
		.note
		.as_deref()
		.map(|n| format!(" · {n}"))
		.unwrap_or_default();
	if state.watching.caught_up && !state.watching.full_pending {
		note.push_str(" · Watcher caught up");
	}
	let refs = list_note(state);
	if state.scanning {
		format!("{lead} · Checking… · {refs}{missing}{note}")
	} else if !clock_shown {
		format!("{lead} · {refs}{missing}{note}")
	} else {
		format!("{lead} · Checked {time} · {refs}{missing}{note}")
	}
}

/// Local wall-clock HH:MM: UTC seconds plus the offset the last scan read, or UTC marked as such
/// when the offset is unknown. Nothing here touches the OS, so the UI thread stays free.
pub(super) fn clock(t: std::time::SystemTime, offset: Option<i32>) -> String {
	let secs = t
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs())
		.unwrap_or(0);
	let Some(offset) = offset else {
		let minutes = (secs as i64).rem_euclid(86_400) / 60;
		return format!("{:02}:{:02} UTC", minutes / 60, minutes % 60);
	};
	let local = secs as i64 + i64::from(offset) * 60;
	let minutes = local.rem_euclid(86_400) / 60;
	format!("{:02}:{:02}", minutes / 60, minutes % 60)
}
