//! The local branches, read-only: `git for-each-ref refs/heads` with the upstream and its saved
//! ahead/behind counts, plus where HEAD is. No fetch; the counts are against saved refs, as the
//! row pills are.

use std::path::Path;

use super::commands::{GitError, Verb, run};
use crate::model::{Branch, Branches, HeadState};

/// One record per branch, NUL between fields and a NUL after each record: full name, short
/// hash, upstream short name, the track summary, and `*` for the checked-out branch.
const FORMAT: &str = "--format=%(refname:short)%00%(objectname:short)%00%(upstream:short)%00%(upstream:track)%00%(HEAD)%00";

pub fn read_branches(repo: &Path) -> Result<Branches, GitError> {
	let bytes = run(repo, Verb::ForEachRef, &[FORMAT, "refs/heads"])?;
	let mut list = parse_branches(&bytes);
	let head = match list.iter().find(|b| b.current) {
		Some(b) => HeadState::OnBranch(b.name.clone()),
		None => match run(repo, Verb::RevParse, &["--short", "HEAD"]) {
			Ok(out) => HeadState::Detached(String::from_utf8_lossy(&out).trim().to_string()),
			Err(_) => HeadState::Unborn,
		},
	};
	// Current first, then by name, bytewise so the order never shifts with the locale.
	list.sort_by(|a, b| b.current.cmp(&a.current).then_with(|| a.name.cmp(&b.name)));
	Ok(Branches { head, list })
}

/// Five NUL-separated fields per record.
pub fn parse_branches(bytes: &[u8]) -> Vec<Branch> {
	let text = String::from_utf8_lossy(bytes);
	let mut fields = text.split('\0');
	let mut out = Vec::new();
	while let Some(name) = fields.next() {
		let name = name.trim_start_matches('\n');
		if name.is_empty() {
			break;
		}
		let (Some(short), Some(upstream), Some(track), Some(head)) =
			(fields.next(), fields.next(), fields.next(), fields.next())
		else {
			break;
		};
		// No upstream means no counts at all; with one, the track summary decides, and an empty
		// summary is level, not unknown (Roger on 0de88a1).
		let (ahead, behind) = if upstream.is_empty() {
			(None, None)
		} else {
			parse_track(track)
		};
		out.push(Branch {
			name: name.to_string(),
			short_hash: short.to_string(),
			upstream: (!upstream.is_empty()).then(|| upstream.to_string()),
			ahead,
			behind,
			current: head == "*",
		});
	}
	out
}

/// `%(upstream:track)` for a branch with an upstream is `[ahead 2, behind 1]`, `[ahead 2]`,
/// `[behind 1]`, empty when level with it, and `[gone]` when the upstream ref no longer exists;
/// the counts are both zero when level and both None when gone. A branch without an upstream
/// never reaches here.
pub fn parse_track(track: &str) -> (Option<u32>, Option<u32>) {
	let inner = track.trim().trim_start_matches('[').trim_end_matches(']');
	if inner == "gone" {
		return (None, None);
	}
	let mut ahead = 0;
	let mut behind = 0;
	for part in inner.split(',') {
		let mut words = part.split_whitespace();
		match (
			words.next(),
			words.next().and_then(|n| n.parse::<u32>().ok()),
		) {
			(Some("ahead"), Some(n)) => ahead = n,
			(Some("behind"), Some(n)) => behind = n,
			_ => {}
		}
	}
	(Some(ahead), Some(behind))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn track_summaries() {
		assert_eq!(parse_track("[ahead 2, behind 1]"), (Some(2), Some(1)));
		assert_eq!(parse_track("[ahead 3]"), (Some(3), Some(0)));
		assert_eq!(parse_track("[behind 4]"), (Some(0), Some(4)));
		assert_eq!(
			parse_track(""),
			(Some(0), Some(0)),
			"level with the upstream"
		);
		assert_eq!(parse_track("[gone]"), (None, None));
	}

	#[test]
	fn records_parse_and_empty_upstream_is_none() {
		let bytes = b"main\0ab12cd3\0origin/main\0[ahead 1]\0*\0\nfeature\0ef45ab6\0\0\0 \0\nlevel\0ab12cd3\0origin/level\0\0 \0\n";
		let list = parse_branches(bytes);
		assert_eq!(list.len(), 3);
		assert_eq!(list[2].name, "level");
		assert_eq!(list[2].upstream.as_deref(), Some("origin/level"));
		assert_eq!(
			(list[2].ahead, list[2].behind),
			(Some(0), Some(0)),
			"an upstream with an empty track summary is level, not unknown"
		);
		assert_eq!(list[0].name, "main");
		assert!(list[0].current);
		assert_eq!(list[0].upstream.as_deref(), Some("origin/main"));
		assert_eq!((list[0].ahead, list[0].behind), (Some(1), Some(0)));
		assert_eq!(list[1].name, "feature");
		assert!(!list[1].current);
		assert_eq!(list[1].upstream, None);
		assert_eq!((list[1].ahead, list[1].behind), (None, None));
	}
}
