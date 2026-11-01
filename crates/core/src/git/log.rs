//! The linear log: `git log -z` with NUL-separated fields, 100 at a time, and a commit's files
//! from `--name-status` with first-parent merges.

use std::collections::HashSet;
use std::path::Path;

use super::commands::{GitError, Verb, run};
use crate::model::{ChangeKind, Commit, CommitFile, parse_iso};

pub const PAGE: usize = 100;
const FORMAT: &str = "--format=%H%x00%h%x00%an%x00%aI%x00%s%x00%b";

/// One page of the current branch's history, newest first.
pub fn read_log(repo: &Path, skip: usize) -> Result<Vec<Commit>, GitError> {
	let max = format!("--max-count={PAGE}");
	let skip_arg = format!("--skip={skip}");
	let bytes = run(repo, Verb::Log, &["-z", FORMAT, &max, &skip_arg])?;
	Ok(parse_log(&bytes))
}

/// Six NUL-separated fields per record; the record's own terminator follows the body.
pub fn parse_log(bytes: &[u8]) -> Vec<Commit> {
	let text = String::from_utf8_lossy(bytes);
	let mut fields = text.split('\0');
	let mut out = Vec::new();
	while let Some(hash) = fields.next() {
		if hash.is_empty() {
			break;
		}
		let (Some(short), Some(author), Some(date), Some(subject), Some(body)) = (
			fields.next(),
			fields.next(),
			fields.next(),
			fields.next(),
			fields.next(),
		) else {
			break;
		};
		out.push(Commit {
			hash: hash.to_string(),
			short: short.to_string(),
			author: author.to_string(),
			time: parse_iso(date).unwrap_or(0),
			date: date.to_string(),
			subject: subject.to_string(),
			body: body.trim_end().to_string(),
		});
	}
	out
}

/// The hashes on HEAD that are not on the upstream: `git log <upstream>..HEAD`. The status call
/// gives the ahead count, not the commits, and after a merge the two differ (Morticia M-2 on
/// 51bc93d). Read-only; no network.
pub fn read_outgoing(repo: &Path, upstream: &str) -> Result<HashSet<String>, GitError> {
	let range = format!("{upstream}..HEAD");
	let bytes = run(repo, Verb::Log, &["-z", "--format=%H", &range, "--"])?;
	Ok(String::from_utf8_lossy(&bytes)
		.split(['\0', '\n'])
		.filter(|h| !h.is_empty())
		.map(str::to_string)
		.collect())
}

/// The files one commit changed against its first parent.
pub fn read_commit_files(repo: &Path, hash: &str) -> Result<Vec<CommitFile>, GitError> {
	let bytes = run(
		repo,
		Verb::Log,
		&[
			"-1",
			"-z",
			"--name-status",
			"--diff-merges=first-parent",
			"--format=",
			hash,
		],
	)?;
	Ok(parse_name_status(&bytes))
}

/// `M\0path\0`, `A\0path\0`, `D\0path\0`, `R100\0old\0new\0`, `C75\0old\0new\0`.
pub fn parse_name_status(bytes: &[u8]) -> Vec<CommitFile> {
	let text = String::from_utf8_lossy(bytes);
	let mut tokens = text.split('\0').filter(|t| !t.is_empty());
	let mut out = Vec::new();
	while let Some(status) = tokens.next() {
		let code = status.as_bytes().first().copied().unwrap_or(b'M');
		let kind = match code {
			b'A' => ChangeKind::Added,
			b'D' => ChangeKind::Deleted,
			b'R' | b'C' => ChangeKind::Renamed,
			b'T' => ChangeKind::TypeChanged,
			_ => ChangeKind::Modified,
		};
		if kind == ChangeKind::Renamed {
			let (Some(from), Some(to)) = (tokens.next(), tokens.next()) else {
				break;
			};
			out.push(CommitFile {
				path: to.to_string(),
				kind,
				from: Some(from.to_string()),
			});
		} else {
			let Some(path) = tokens.next() else { break };
			out.push(CommitFile {
				path: path.to_string(),
				kind,
				from: None,
			});
		}
	}
	out
}
