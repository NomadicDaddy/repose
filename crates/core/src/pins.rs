//! Pinned repositories (repository-pinning clauses 1, 2 and 5): the list in config.toml's
//! `pinned_repositories`, matched to discovered paths without regard to case or slash direction,
//! and saved by rewriting that one key in place, so roots, excludes, depth, commands, every other
//! key and every comment stay exactly as they were. Nothing here runs git.

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::ConfigError;
use crate::model::path_key;

/// Whether `path` is in `pins`, comparing as Windows does: case and `/` or `\` don't matter.
pub fn is_pinned(pins: &[PathBuf], path: &Path) -> bool {
	let key = path_key(path);
	pins.iter().any(|p| path_key(p) == key)
}

/// `pins` with `path` added, unless an equal path is already there.
pub fn pinned(pins: &[PathBuf], path: &Path) -> Vec<PathBuf> {
	let mut out = pins.to_vec();
	if !is_pinned(pins, path) {
		out.push(path.to_path_buf());
	}
	out
}

/// `pins` with every path equal to `path` removed.
pub fn unpinned(pins: &[PathBuf], path: &Path) -> Vec<PathBuf> {
	let key = path_key(path);
	pins.iter()
		.filter(|p| path_key(p) != key)
		.cloned()
		.collect()
}

/// Writes `pins` as `pinned_repositories` in the config file at `path`, changing nothing else in
/// it. The new text goes to a sibling file first and then replaces the config in one rename, so a
/// failed write leaves the old file whole; the error names the file.
pub fn save(path: &Path, pins: &[PathBuf]) -> Result<(), ConfigError> {
	let text = fs::read_to_string(path).map_err(|e| ConfigError::Io(path.to_path_buf(), e))?;
	let write = |e| ConfigError::Write(path.to_path_buf(), e);
	let mut doc: toml_edit::DocumentMut = text
		.parse()
		.map_err(|e: toml_edit::TomlError| ConfigError::Edit(path.to_path_buf(), e.to_string()))?;
	let mut list = toml_edit::Array::new();
	for pin in pins {
		list.push(pin.to_string_lossy().as_ref());
	}
	doc["pinned_repositories"] = toml_edit::value(list);
	let staged = path.with_extension("toml.saving");
	fs::write(&staged, doc.to_string()).map_err(write)?;
	fs::rename(&staged, path).map_err(|e| {
		let _ = fs::remove_file(&staged);
		write(e)
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pins_match_without_regard_to_case_or_slashes_and_never_repeat() {
		let pins = vec![PathBuf::from("D:\\Applications\\Repose")];
		assert!(is_pinned(&pins, Path::new("d:/applications/repose")));
		assert!(is_pinned(&pins, Path::new("D:\\applications\\repose\\")));
		assert!(!is_pinned(&pins, Path::new("D:\\applications\\repose2")));
		assert_eq!(pinned(&pins, Path::new("d:/applications/repose")), pins);
		let two = pinned(&pins, Path::new("D:\\bbs"));
		assert_eq!(two.len(), 2);
		assert_eq!(
			unpinned(&two, Path::new("d:/APPLICATIONS/repose")),
			vec![PathBuf::from("D:\\bbs")]
		);
	}
}
