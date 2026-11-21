//! The text a suggestion sends (A1): the fixed instruction, the style hints when they are on, the
//! staged file list and the staged diff, kept under `CAP` with the hints counted inside it.

use super::{CAP, INSTRUCTION};

/// The text sent on stdin, and the staged files whose diffs did not fit under `CAP`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
	pub text: String,
	pub left_out: Vec<String>,
}

/// The staged changes as git names them, in `git diff --cached` order, and that diff, both from
/// one read (`git::diff::staged_for_suggestion`). Each file is its label and how many diff
/// sections git prints for it: two for a type change, which it shows as a delete then an add.
/// `subjects` are the style hints (`git::log::recent_subjects`), empty when they are off.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Staged {
	pub files: Vec<crate::git::diff::StagedEntry>,
	pub diff: Vec<u8>,
	pub subjects: Vec<String>,
}

/// Builds the request. The style hints, when there are any, come right after the instruction and
/// before any diff, and they count inside `CAP` (ai-commit-message-style-hints). Every staged file
/// is listed by its label. Whole-file diffs follow in git's order until the next one would take
/// the hints and diffs past `CAP`; a file's diff (both sections of a type change) is never cut
/// part-way, and the files left out are named in the request and returned. Sections are matched
/// to files by position; names never come from headers (Roger 5Q-2 on d3da4dd). One git read
/// keeps them consistent (Roger 5Q-3 on 987f892); a diff whose section count is not what the list
/// says is refused.
pub fn request(staged: &Staged) -> Result<Request, String> {
	let sections = split(&staged.diff);
	let expected: usize = staged.files.iter().map(|(_, n)| n).sum();
	if sections.len() != expected {
		return Err(format!(
			"git's staged list needs {expected} diff sections but its diff has {}; nothing was sent",
			sections.len()
		));
	}
	let hints = hints(&staged.subjects);
	let mut kept = String::new();
	let mut left_out = Vec::new();
	let mut parts = sections.into_iter();
	for (label, count) in &staged.files {
		let body: String = parts.by_ref().take(*count).collect();
		if left_out.is_empty() && hints.len() + kept.len() + body.len() <= CAP {
			kept.push_str(&body);
		} else {
			left_out.push(label.clone());
		}
	}
	let mut text = format!("{INSTRUCTION}\n\n{hints}Staged files:\n");
	for (label, _) in &staged.files {
		text.push_str(&format!("- {label}\n"));
	}
	if !left_out.is_empty() {
		text.push_str(&format!(
			"\nThe diffs of {} of these files are left out to stay under {} KiB: {}.\n",
			left_out.len(),
			CAP / 1024,
			left_out.join(", ")
		));
	}
	text.push_str("\nDiff:\n");
	text.push_str(&kept);
	Ok(Request { text, left_out })
}

/// The style-hints block: the recent subjects, one per line, or nothing at all when there are none.
/// A subject that would take the block past `CAP` ends it, so the hints alone never break the cap.
fn hints(subjects: &[String]) -> String {
	const HEADER: &str =
		"Recent commit subjects on this branch, newest first; match their style:\n";
	let mut block = String::from(HEADER);
	for subject in subjects {
		let line = format!("- {subject}\n");
		if block.len() + line.len() + 1 > CAP {
			break;
		}
		block.push_str(&line);
	}
	if block.len() == HEADER.len() {
		return String::new();
	}
	block.push('\n');
	block
}

/// The diff cut into its file sections, each starting at a `diff --git ` line.
fn split(diff: &[u8]) -> Vec<String> {
	let text = String::from_utf8_lossy(diff);
	let mut sections: Vec<String> = Vec::new();
	for line in text.split_inclusive('\n') {
		if line.starts_with("diff --git ") {
			sections.push(String::new());
		}
		if let Some(body) = sections.last_mut() {
			body.push_str(line);
		}
	}
	sections
}
