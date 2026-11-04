//! Flattens a parsed diff into drawable rows for the two modes, and computes word-level
//! highlights for changed line pairs with `similar`. Pure functions; drawing is in diff.rs.

use std::ops::Range;
use std::time::{Duration, Instant};

use repose_core::model::{DiffLine, FileDiff, Hunk, LineKind};
use similar::{ChangeTag, TextDiff};

/// Lines longer than this get no word marks: a minified bundle or one-line JSON would otherwise
/// hold the thread for seconds (Morticia M-1 on 4a6b980: 235 KB took 3.6 s, 1.8 MB over 40 s).
pub const MAX_MARK_BYTES: usize = 4 * 1024;
/// Past this the word diff returns what it has, which may be coarser but never hangs.
const MARK_DEADLINE: Duration = Duration::from_millis(20);

/// Where a line lives in the parsed diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRef {
	pub hunk: usize,
	pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
	/// A hunk boundary: how many unchanged lines were skipped before it, and git's heading.
	Separator { skipped: u32, heading: String },
	/// Side by side: the old and new cells, with byte ranges to highlight in each text.
	Pair {
		old: Option<LineRef>,
		new: Option<LineRef>,
		old_marks: Vec<Range<usize>>,
		new_marks: Vec<Range<usize>>,
	},
	/// Inline: one line with its highlights.
	Line {
		line: LineRef,
		marks: Vec<Range<usize>>,
	},
}

pub fn line(file: &FileDiff, r: LineRef) -> &DiffLine {
	&file.hunks[r.hunk].lines[r.line]
}

pub fn build_rows(file: &FileDiff, inline: bool) -> Vec<Row> {
	let mut rows = Vec::new();
	let mut last_old_end = 1u32;
	for (hi, hunk) in file.hunks.iter().enumerate() {
		rows.push(Row::Separator {
			skipped: hunk.old_start.saturating_sub(last_old_end),
			heading: hunk.heading.clone(),
		});
		last_old_end = hunk.old_start + hunk.old_count;
		if inline {
			inline_rows(hunk, hi, &mut rows);
		} else {
			paired_rows(hunk, hi, &mut rows);
		}
	}
	rows
}

/// Runs of removed lines followed by added lines pair up position by position; the longer side's
/// tail pairs with an empty cell. Context lines pair with themselves.
fn paired_rows(hunk: &Hunk, hi: usize, rows: &mut Vec<Row>) {
	let lines = &hunk.lines;
	let mut i = 0;
	while i < lines.len() {
		match lines[i].kind {
			LineKind::Context => {
				let r = LineRef { hunk: hi, line: i };
				rows.push(Row::Pair {
					old: Some(r),
					new: Some(r),
					old_marks: Vec::new(),
					new_marks: Vec::new(),
				});
				i += 1;
			}
			_ => {
				let start = i;
				while i < lines.len() && lines[i].kind == LineKind::Removed {
					i += 1;
				}
				let removed = start..i;
				let add_start = i;
				while i < lines.len() && lines[i].kind == LineKind::Added {
					i += 1;
				}
				let added = add_start..i;
				let n = removed.len().max(added.len());
				for k in 0..n {
					let old = removed
						.clone()
						.nth(k)
						.map(|l| LineRef { hunk: hi, line: l });
					let new = added.clone().nth(k).map(|l| LineRef { hunk: hi, line: l });
					let (old_marks, new_marks) = match (old, new) {
						(Some(o), Some(a)) => word_marks(&lines[o.line].text, &lines[a.line].text),
						_ => (Vec::new(), Vec::new()),
					};
					rows.push(Row::Pair {
						old,
						new,
						old_marks,
						new_marks,
					});
				}
			}
		}
	}
}

/// Inline keeps git's order; a removed run followed by an added run of the same length gets
/// word marks pair by pair, which is the common one-line edit.
fn inline_rows(hunk: &Hunk, hi: usize, rows: &mut Vec<Row>) {
	let lines = &hunk.lines;
	let mut marks: Vec<Vec<Range<usize>>> = vec![Vec::new(); lines.len()];
	let mut i = 0;
	while i < lines.len() {
		if lines[i].kind == LineKind::Removed {
			let start = i;
			while i < lines.len() && lines[i].kind == LineKind::Removed {
				i += 1;
			}
			let add_start = i;
			while i < lines.len() && lines[i].kind == LineKind::Added {
				i += 1;
			}
			let (removed, added) = (start..add_start, add_start..i);
			if removed.len() == added.len() {
				for (o, a) in removed.zip(added) {
					let (om, am) = word_marks(&lines[o].text, &lines[a].text);
					marks[o] = om;
					marks[a] = am;
				}
			}
		} else {
			i += 1;
		}
	}
	for (li, m) in marks.into_iter().enumerate() {
		rows.push(Row::Line {
			line: LineRef { hunk: hi, line: li },
			marks: m,
		});
	}
}

/// Byte ranges that differ between two lines, by words. Identical lines get none, and a pair with
/// nothing in common is left unmarked rather than fully painted.
pub fn word_marks(old: &str, new: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
	if old == new || old.len() > MAX_MARK_BYTES || new.len() > MAX_MARK_BYTES {
		return (Vec::new(), Vec::new());
	}
	let diff = TextDiff::configure()
		.deadline(Instant::now() + MARK_DEADLINE)
		.diff_words(old, new);
	let (mut o, mut n) = (0usize, 0usize);
	let (mut old_marks, mut new_marks) = (Vec::new(), Vec::new());
	let mut equal_bytes = 0usize;
	for change in diff.iter_all_changes() {
		let len = change.value().len();
		match change.tag() {
			ChangeTag::Equal => {
				equal_bytes += len;
				o += len;
				n += len;
			}
			ChangeTag::Delete => {
				push_merged(&mut old_marks, o..o + len);
				o += len;
			}
			ChangeTag::Insert => {
				push_merged(&mut new_marks, n..n + len);
				n += len;
			}
		}
	}
	if equal_bytes == 0 {
		return (Vec::new(), Vec::new());
	}
	(old_marks, new_marks)
}

fn push_merged(marks: &mut Vec<Range<usize>>, r: Range<usize>) {
	if let Some(last) = marks.last_mut()
		&& last.end == r.start
	{
		last.end = r.end;
	} else {
		marks.push(r);
	}
}

#[cfg(test)]
#[path = "diff_rows_tests.rs"]
mod tests;
