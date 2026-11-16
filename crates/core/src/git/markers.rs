//! Whether a conflicted file still holds conflict markers, read from the working copy when the
//! repository's status is read (conflict-stage-waits-for-markers). It never stages anything.

use std::path::Path;

use crate::model::MarkerCheck;

/// Markers remain while a line starts `<<<<<<< ` or `>>>>>>> `. A lone `=======` counts only
/// beside one of those, so it adds nothing to the answer: alone it is ordinary content, such as a
/// Markdown or reStructuredText heading underline, and must not hold a resolved file back
/// (Morticia Low 1 on 5904afc). A file that cannot be read (missing, locked) or holds a NUL byte
/// (binary) cannot be checked.
pub fn check_markers(file: &Path) -> MarkerCheck {
	let Ok(bytes) = std::fs::read(file) else {
		return MarkerCheck::Unchecked;
	};
	if bytes.contains(&0) {
		return MarkerCheck::Unchecked;
	}
	let marked = bytes.split(|&b| b == b'\n').any(|line| {
		let line = line.strip_suffix(b"\r").unwrap_or(line);
		line.starts_with(b"<<<<<<< ") || line.starts_with(b">>>>>>> ")
	});
	if marked {
		MarkerCheck::Present
	} else {
		MarkerCheck::Clear
	}
}
