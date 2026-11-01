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
