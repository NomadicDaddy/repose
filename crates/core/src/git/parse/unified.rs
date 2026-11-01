//! Parses `git diff` output produced with explicit `--src-prefix=a/ --dst-prefix=b/`, so the
//! prefixes are known whatever `diff.noprefix` or `diff.mnemonicPrefix` says in a repository.
//! Fixtures in `tests/diff.rs` come from real git; the unit tests here cover shapes that are hard
//! to produce on demand.

use crate::model::{DiffLine, FileDiff, Hunk, LineKind};

const SRC: &str = "a/";
const DST: &str = "b/";

pub fn parse_unified(bytes: &[u8]) -> Vec<FileDiff> {
	let text = String::from_utf8_lossy(bytes);
	let mut files: Vec<FileDiff> = Vec::new();
	let mut hunk: Option<Hunk> = None;
	let (mut old_no, mut new_no) = (0u32, 0u32);

	for raw in text.split_inclusive('\n') {
		// Body lines keep their CR so a CRLF-to-LF change reads as a change; headers never
		// carry one from git, and dropping it there is harmless.
		let body = raw.strip_suffix('\n').unwrap_or(raw);
		let line = body.strip_suffix('\r').unwrap_or(body);
		if let Some(rest) = line.strip_prefix("diff --git ") {
			close_hunk(&mut files, &mut hunk);
			files.push(file_from_header(rest));
			continue;
		}
		if let Some(path) = line.strip_prefix("diff --cc ") {
			// A conflicted file: git prints a combined diff, which this viewer does not draw.
			close_hunk(&mut files, &mut hunk);
			files.push(FileDiff {
				old_path: Some(path.to_string()),
				new_path: Some(path.to_string()),
				conflicted: true,
				..FileDiff::default()
			});
			continue;
		}
		let Some(file) = files.last_mut() else {
			continue;
		};
		if let Some(h) = hunk.as_mut()
			&& let Some(parsed) = body_line(body, &mut old_no, &mut new_no)
		{
			h.lines.push(parsed);
			continue;
		}
		if line.starts_with("\\ No newline") {
			continue;
		}
		if let Some(rest) = line.strip_prefix("@@ ") {
			close_hunk(&mut files, &mut hunk);
			if let Some(h) = hunk_header(rest) {
				old_no = h.old_start;
				new_no = h.new_start;
				hunk = Some(h);
			}
			continue;
		}
		if let Some(p) = line.strip_prefix("rename from ") {
			file.old_path = Some(p.to_string());
		} else if let Some(p) = line.strip_prefix("rename to ") {
			file.new_path = Some(p.to_string());
		} else if let Some(p) = line.strip_prefix("--- ") {
			file.old_path = path_from_marker(p, SRC);
		} else if let Some(p) = line.strip_prefix("+++ ") {
			file.new_path = path_from_marker(p, DST);
		} else if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
			file.binary = true;
		}
	}
	close_hunk(&mut files, &mut hunk);
	files
}

fn close_hunk(files: &mut [FileDiff], hunk: &mut Option<Hunk>) {
	if let (Some(h), Some(file)) = (hunk.take(), files.last_mut()) {
		file.hunks.push(h);
	}
}

/// `a/path b/path`. For anything but a rename the two halves are the same path, so the exact
/// split is the middle; a path containing ` b/` cannot fool that. A rename's header is split at
/// the last ` b/`, and its `rename from` and `rename to` lines then set both paths exactly.
/// Quoted paths (non-ASCII under core.quotePath=true) do not occur: every spawn sets
/// core.quotePath=false.
fn file_from_header(rest: &str) -> FileDiff {
	let mut file = FileDiff::default();
	let bytes = rest.as_bytes();
	if bytes.len() % 2 == 1 {
		let mid = bytes.len() / 2;
		if bytes[mid] == b' '
			&& rest.is_char_boundary(mid)
			&& rest[..mid].strip_prefix(SRC) == rest[mid + 1..].strip_prefix(DST)
			&& let Some(path) = rest[..mid].strip_prefix(SRC)
		{
			file.old_path = Some(path.to_string());
			file.new_path = Some(path.to_string());
			return file;
		}
	}
	if let Some(i) = rest.rfind(" b/") {
		let old = rest[..i].strip_prefix(SRC).unwrap_or(&rest[..i]);
		let new = &rest[i + 3..];
		file.old_path = Some(old.to_string());
		file.new_path = Some(new.to_string());
	}
	file
}

/// `a/path`, `b/path`, or `/dev/null`. Git appends a tab when the path contains whitespace
/// (Morticia M-1 on 94c9dd0); it is not part of the name.
fn path_from_marker(marker: &str, prefix: &str) -> Option<String> {
	let marker = marker.strip_suffix('\t').unwrap_or(marker);
	if marker == "/dev/null" {
		return None;
	}
	Some(marker.strip_prefix(prefix).unwrap_or(marker).to_string())
}

/// `-12,7 +12,8 @@ fn name()` with the counts optional.
fn hunk_header(rest: &str) -> Option<Hunk> {
	let (ranges, heading) = rest.split_once(" @@")?;
	let (old, new) = ranges.split_once(' ')?;
	let (old_start, old_count) = range(old.strip_prefix('-')?)?;
	let (new_start, new_count) = range(new.strip_prefix('+')?)?;
	Some(Hunk {
		old_start,
		old_count,
		new_start,
		new_count,
		heading: heading.trim_start().to_string(),
		lines: Vec::new(),
	})
}

fn range(text: &str) -> Option<(u32, u32)> {
	match text.split_once(',') {
		Some((s, c)) => Some((s.parse().ok()?, c.parse().ok()?)),
		None => Some((text.parse().ok()?, 1)),
	}
}

fn body_line(line: &str, old_no: &mut u32, new_no: &mut u32) -> Option<DiffLine> {
	let (kind, text) = match line.as_bytes().first() {
		Some(b' ') => (LineKind::Context, &line[1..]),
		Some(b'+') => (LineKind::Added, &line[1..]),
		Some(b'-') => (LineKind::Removed, &line[1..]),
		None => (LineKind::Context, ""),
		_ => return None,
	};
	let (o, n) = match kind {
		LineKind::Context => {
			let pair = (Some(*old_no), Some(*new_no));
			*old_no += 1;
			*new_no += 1;
			pair
		}
		LineKind::Added => {
			let pair = (None, Some(*new_no));
			*new_no += 1;
			pair
		}
		LineKind::Removed => {
			let pair = (Some(*old_no), None);
			*old_no += 1;
			pair
		}
	};
	Some(DiffLine {
		kind,
		old_no: o,
		new_no: n,
		text: text.to_string(),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rename_with_edit_and_hunk_numbers() {
		let text = "diff --git a/old.txt b/new.txt\nsimilarity index 80%\nrename from old.txt\nrename to new.txt\nindex 1..2 100644\n--- a/old.txt\n+++ b/new.txt\n@@ -1,3 +1,3 @@ heading\n one\n-two\n+TWO\n three\n";
		let files = parse_unified(text.as_bytes());
		assert_eq!(files.len(), 1);
		let f = &files[0];
		assert!(f.is_rename());
		assert_eq!(f.old_path.as_deref(), Some("old.txt"));
		assert_eq!(f.new_path.as_deref(), Some("new.txt"));
		let h = &f.hunks[0];
		assert_eq!(h.heading, "heading");
		assert_eq!((h.lines[1].old_no, h.lines[1].new_no), (Some(2), None));
		assert_eq!((h.lines[2].old_no, h.lines[2].new_no), (None, Some(2)));
		assert_eq!((f.added(), f.removed()), (1, 1));
	}

	#[test]
	fn added_deleted_binary_and_a_body_line_that_looks_like_a_header() {
		let text = "diff --git a/n.txt b/n.txt\nnew file mode 100644\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1,2 @@\n+hi\n+++ plus\ndiff --git a/g.sql b/g.sql\ndeleted file mode 100644\n--- a/g.sql\n+++ /dev/null\n@@ -1 +0,0 @@\n--- sql comment\n\\ No newline at end of file\ndiff --git a/p.png b/p.png\nBinary files a/p.png and b/p.png differ\n";
		let files = parse_unified(text.as_bytes());
		assert_eq!(files.len(), 3);
		assert_eq!(files[0].old_path, None);
		assert_eq!(files[0].hunks[0].lines[1].text, "++ plus");
		assert_eq!(files[1].new_path, None);
		assert_eq!(files[1].hunks[0].lines[0].text, "-- sql comment");
		assert!(files[2].binary);
	}

	#[test]
	fn crlf_change_keeps_the_cr_and_a_combined_diff_is_flagged() {
		let text = "diff --git a/w.txt b/w.txt\n--- a/w.txt\n+++ b/w.txt\n@@ -1 +1 @@\n-one\r\n+one\ndiff --cc c.txt\nindex 1,2..0\n--- a/c.txt\n+++ b/c.txt\n@@@ -1,1 -1,1 +1,5 @@@\n++<<<<<<< HEAD\n";
		let files = parse_unified(text.as_bytes());
		assert_eq!(files.len(), 2);
		let lines = &files[0].hunks[0].lines;
		assert_eq!(lines[0].text, "one\r");
		assert_eq!(lines[1].text, "one");
		assert!(files[1].conflicted);
		assert_eq!(files[1].path(), "c.txt");
		assert!(files[1].hunks.is_empty());
	}
}
