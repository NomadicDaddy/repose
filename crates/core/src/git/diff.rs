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
	let bytes = run(repo, verb, &args)?;
	Ok(parse_unified(&bytes))
}

/// The size of a blob as the index or a commit holds it, for the too-large check before a diff of
/// the staged or committed side. `rev` is `:0` for the index or a commit hash.
pub fn blob_size(repo: &Path, rev: &str, path: &str) -> Result<u64, GitError> {
	let spec = format!("{rev}:{path}");
	let out = run(repo, Verb::CatFile, &["-s", &spec])?;
	String::from_utf8_lossy(&out)
		.trim()
		.parse::<u64>()
		.map_err(|e| GitError::Spawn(format!("cat-file -s gave no size: {e}")))
}
