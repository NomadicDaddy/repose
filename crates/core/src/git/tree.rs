//! A committed tree and its files, read-only: `ls-tree -r -l -z` for the listing (the sizes come
//! with it, so the too-large check costs no second spawn) and `cat-file -p <rev>:<path>` for one
//! file. Neither touches the working tree or the index.

use std::path::Path;

use super::commands::{GitError, Verb, run};
use super::parse::parse_ls_tree;
use crate::model::TreeEntry;

/// Every file under `rev` (`HEAD` or a commit hash), recursively, in git's own path order.
pub fn read_tree(repo: &Path, rev: &str) -> Result<Vec<TreeEntry>, GitError> {
	let bytes = run(repo, Verb::LsTree, &["-r", "-l", "-z", rev, "--"])?;
	Ok(parse_ls_tree(&bytes))
}

/// The bytes of one file as `rev` holds them. `rev:path` is an object name git resolves itself,
/// so no pathspec is involved and a path with spaces or brackets needs no quoting.
pub fn read_file(repo: &Path, rev: &str, path: &str) -> Result<Vec<u8>, GitError> {
	let spec = format!("{rev}:{path}");
	run(repo, Verb::CatFile, &["-p", &spec])
}

/// Git's own rule for its diffs: a NUL within the first 8000 bytes makes a file binary.
pub fn is_binary(bytes: &[u8]) -> bool {
	bytes.iter().take(8000).any(|b| *b == 0)
}
