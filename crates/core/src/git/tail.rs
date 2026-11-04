//! The kept tail of git's stderr for errors and the window: the last lines, bounded in bytes.

pub(super) fn tail(bytes: &[u8]) -> String {
	tail_lines(bytes, 3, " | ")
}

/// The most a kept tail may hold, whatever its line count: one enormous line from a hook is
/// cut to its last 8 KB rather than copied whole into the error and the window (Roger 5Q-2 on
/// 9a05e06).
const TAIL_BYTES: usize = 8 * 1024;

/// The last `n` lines of git's stderr, joined by `sep`, in order, and at most `TAIL_BYTES`.
pub(super) fn tail_lines(bytes: &[u8], n: usize, sep: &str) -> String {
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
