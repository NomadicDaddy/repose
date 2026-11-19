//! Read-only diffs: working tree against index, index against HEAD, and a commit against its first
//! parent, all parsed from unified output with explicit prefixes.

use std::path::Path;

use super::commands::{GitError, Verb, run};
use super::parse::parse_unified;
use crate::model::{DiffSource, FileDiff};

/// Shared by every diff spawn. The prefixes are explicit so a repository's diff.noprefix or
/// diff.mnemonicPrefix cannot change what the parser reads; no external diff and no textconv
/// program, so a repository's config cannot run anything from a click (same class as fsmonitor).
const COMMON: [&str; 7] = [
	"--no-color",
	"--no-ext-diff",
	"--no-textconv",
	"--find-renames",
	"--src-prefix=a/",
	"--dst-prefix=b/",
	"-U3",
];

/// Files over this many bytes are not diffed or highlighted; the view says so instead.
pub const MAX_DIFF_BYTES: u64 = 2 * 1024 * 1024;

/// The diff of the given paths (both paths of a rename) from one source. Empty paths means every
/// file, which the file-level views never ask for.
pub fn read_diff(
	repo: &Path,
	source: &DiffSource,
	paths: &[String],
) -> Result<Vec<FileDiff>, GitError> {
	Ok(parse_unified(&read_diff_bytes(repo, source, paths)?))
}

/// The same diff as git prints it, before parsing, so a caller that re-reads on every status
/// can tell an unchanged diff from a changed one by its bytes (Morticia M-1 on 9007389: a
/// second edit to an already-modified file changes the diff and nothing in the status).
pub fn read_diff_bytes(
	repo: &Path,
	source: &DiffSource,
	paths: &[String],
) -> Result<Vec<u8>, GitError> {
	let mut args: Vec<&str> = Vec::new();
	let verb = match source {
		DiffSource::Working => Verb::Diff,
		DiffSource::Staged => {
			args.push("--cached");
			Verb::Diff
		}
		DiffSource::Commit(_) => {
			args.push("--format=");
			args.push("--diff-merges=first-parent");
			Verb::Show
		}
	};
	args.extend(COMMON);
	if let DiffSource::Commit(hash) = source {
		args.push(hash);
	}
	args.push("--");
	args.extend(paths.iter().map(String::as_str));
	run(repo, verb, &args)
}

/// The size of a blob as the index or a commit holds it, for the too-large check before a diff of
/// the staged or committed side. `rev` is `:0` for the index, `HEAD`, a commit hash or `<hash>^`.
/// `Ok(None)` means the object does not exist there: the path is absent, HEAD is unborn, the
/// commit has no parent, or a conflicted path has no stage-0 entry. That answer comes from
/// `rev-parse --verify --quiet`'s exit code and never from git's message text, which differs
/// between versions (Morticia M-2 on 15ce293).
pub fn blob_size(repo: &Path, rev: &str, path: &str) -> Result<Option<u64>, GitError> {
	let spec = format!("{rev}:{path}");
	let oid = match run(repo, Verb::RevParse, &["--verify", "--quiet", &spec]) {
		Ok(out) => String::from_utf8_lossy(&out).trim().to_string(),
		Err(GitError::Failed {
			code: Some(1),
			ref stderr_tail,
		}) if stderr_tail.trim().is_empty() => return Ok(None),
		Err(e) => return Err(e),
	};
	let out = run(repo, Verb::CatFile, &["-s", &oid])?;
	String::from_utf8_lossy(&out)
		.trim()
		.parse::<u64>()
		.map(Some)
		.map_err(|e| GitError::Spawn(format!("cat-file -s gave no size: {e}")))
}

/// The staged changes a commit-message suggestion sends, read by one `git diff --cached` so the
/// file list and the diff always describe the same index (Roger 5Q-3 on 987f892): the Changes
/// view's flags, plus `--submodule=short`, so a staged submodule shows only its two commit ids
/// whatever `diff.submodule` says (Morticia M-1 on d3da4dd), and `--patch-with-raw -z`, so each
/// file is first named in git's raw form, never quoted. Returns one label per staged file, the
/// path or `old → new` for a rename or copy, and the patch.
pub fn staged_for_suggestion(repo: &Path) -> Result<(Vec<String>, Vec<u8>), GitError> {
	let mut args = vec!["--cached"];
	args.extend(COMMON);
	args.extend(["--submodule=short", "--patch-with-raw", "-z", "--"]);
	Ok(split_raw(&run(repo, Verb::Diff, &args)?))
}

/// `--patch-with-raw -z` output split into its labels and its patch. Each raw entry is
/// `:<modes> <ids> <status>` and a NUL, then its path and a NUL, with a second path for R and C;
/// one more NUL ends the list, and the patch follows.
pub fn split_raw(out: &[u8]) -> (Vec<String>, Vec<u8>) {
	let mut labels = Vec::new();
	let mut at = 0;
	while out.get(at) == Some(&b':') {
		let Some((meta, next)) = field(out, at) else {
			break;
		};
		let status = meta.rsplit(|&b| b == b' ').next().unwrap_or_default();
		let Some((first, next)) = field(out, next) else {
			break;
		};
		let first = String::from_utf8_lossy(first).to_string();
		at = next;
		if status.first().is_some_and(|s| matches!(s, b'R' | b'C')) {
			let Some((second, next)) = field(out, at) else {
				break;
			};
			labels.push(format!("{first} → {}", String::from_utf8_lossy(second)));
			at = next;
		} else {
			labels.push(first);
		}
	}
	if out.get(at) == Some(&0) {
		at += 1;
	}
	(labels, out.get(at..).unwrap_or_default().to_vec())
}

/// The bytes from `from` to the next NUL, and the index after it.
fn field(out: &[u8], from: usize) -> Option<(&[u8], usize)> {
	let rest = out.get(from..)?;
	let end = rest.iter().position(|&b| b == 0)?;
	Some((&rest[..end], from + end + 1))
}
