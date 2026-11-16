//! Whether a conflicted file still holds conflict markers, read from the working copy when the
//! repository's status is read (conflict-stage-waits-for-markers). It never stages anything.

use std::path::Path;

use crate::model::MarkerCheck;

/// A line that starts `<<<<<<< ` or `>>>>>>> `, or is `=======` alone, is a marker. A file that
/// cannot be read (missing, locked) or holds a NUL byte (binary) cannot be checked.
pub fn check_markers(file: &Path) -> MarkerCheck {
	let Ok(bytes) = std::fs::read(file) else {
		return MarkerCheck::Unchecked;
	};
	if bytes.contains(&0) {
		return MarkerCheck::Unchecked;
	}
	let marked = bytes.split(|&b| b == b'\n').any(|line| {
		let line = line.strip_suffix(b"\r").unwrap_or(line);
		line.starts_with(b"<<<<<<< ") || line.starts_with(b">>>>>>> ") || line == b"======="
	});
	if marked {
		MarkerCheck::Present
	} else {
		MarkerCheck::Clear
	}
}
