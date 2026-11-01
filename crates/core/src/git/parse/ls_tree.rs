//! `git ls-tree -r -l -z`: one record per entry, `<mode> <type> <object> <size>\t<path>\0`, the
//! size right-aligned with spaces and `-` for anything without a blob.

use crate::model::TreeEntry;

/// Entries in git's order. A record that does not have the four header fields and a path is
/// skipped rather than guessed at; trees are not listed by `-r` and are skipped if they appear.
pub fn parse_ls_tree(bytes: &[u8]) -> Vec<TreeEntry> {
	let text = String::from_utf8_lossy(bytes);
	text.split('\0')
		.filter(|record| !record.is_empty())
		.filter_map(|record| {
			let (header, path) = record.split_once('\t')?;
			let mut fields = header.split_whitespace();
			let _mode = fields.next()?;
			let kind = fields.next()?;
			let _object = fields.next()?;
			let size = fields.next()?;
			if kind == "tree" {
				return None;
			}
			Some(TreeEntry {
				path: path.to_string(),
				size: size.parse().unwrap_or(0),
				submodule: kind == "commit",
			})
		})
		.collect()
}
