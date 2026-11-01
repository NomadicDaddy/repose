//! Parses `git diff` output produced with explicit `--src-prefix=a/ --dst-prefix=b/`, so the
//! prefixes are known whatever `diff.noprefix` or `diff.mnemonicPrefix` says in a repository.

use crate::model::{DiffLine, FileDiff, Hunk, LineKind};

const SRC: &str = "a/";
const DST: &str = "b/";

pub fn parse_unified(bytes: &[u8]) -> Vec<FileDiff> {
	let text = String::from_utf8_lossy(bytes);
	let mut files: Vec<FileDiff> = Vec::new();
	let mut hunk: Option<Hunk> = None;
	let (mut old_no, mut new_no) = (0u32, 0u32);

	for line in text.split_inclusive('\n') {
		let line = line.strip_suffix('\n').unwrap_or(line);
		let line = line.strip_suffix('\r').unwrap_or(line);
		if let Some(rest) = line.strip_prefix("diff --git ") {
			close_hunk(&mut files, &mut hunk);
			files.push(file_from_header(rest));
			continue;
		}
		let Some(file) = files.last_mut() else {
			continue;
		};
		if let Some(h) = hunk.as_mut()
			&& let Some(parsed) = body_line(line, &mut old_no, &mut new_no)
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

/// `a/old b/new`; with explicit prefixes the split is at ` b/`, which also survives a space in
/// the path unless the old path itself contains ` b/`. A quoted path (non-ASCII under
/// core.quotePath=true) is not produced because every spawn sets core.quotePath=false.
fn file_from_header(rest: &str) -> FileDiff {
	let mut file = FileDiff::default();
	if let Some(i) = rest.rfind(" b/") {
		let old = rest[..i].strip_prefix(SRC).unwrap_or(&rest[..i]);
		let new = &rest[i + 3..];
		file.old_path = Some(old.to_string());
		file.new_path = Some(new.to_string());
	}
	file
}

/// `a/path`, `b/path`, or `/dev/null`; a trailing tab and timestamp never appear from git.
fn path_from_marker(marker: &str, prefix: &str) -> Option<String> {
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
		assert_eq!(f.hunks.len(), 1);
		let h = &f.hunks[0];
		assert_eq!(h.heading, "heading");
		assert_eq!(h.lines[1].old_no, Some(2));
		assert_eq!(h.lines[1].new_no, None);
		assert_eq!(h.lines[2].new_no, Some(2));
		assert_eq!(h.lines[3].old_no, Some(3));
		assert_eq!((f.added(), f.removed()), (1, 1));
	}

	#[test]
	fn added_deleted_and_binary() {
		let text = "diff --git a/n.txt b/n.txt\nnew file mode 100644\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1 @@\n+hi\ndiff --git a/g.txt b/g.txt\ndeleted file mode 100644\n--- a/g.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-bye\n\\ No newline at end of file\ndiff --git a/p.png b/p.png\nBinary files a/p.png and b/p.png differ\n";
		let files = parse_unified(text.as_bytes());
		assert_eq!(files.len(), 3);
		assert_eq!(files[0].old_path, None);
		assert_eq!(files[0].hunks[0].lines[0].new_no, Some(1));
		assert_eq!(files[1].new_path, None);
		assert_eq!(files[1].path(), "g.txt");
		assert!(files[2].binary);
		assert!(files[2].hunks.is_empty());
	}
}
