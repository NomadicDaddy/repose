//! The size check before a diff is read: both sides measured, through the file system or the
//! object store, and the read refused when a side cannot be measured at all.

use std::path::Path;

use repose_core::git::diff::blob_size;
use repose_core::model::DiffSource;

use super::OpenDiff;

/// One side of a diff as the size check sees it.
enum Side {
	Size(u64),
	/// The side does not exist: an added or deleted file. Git or the file system said so.
	Absent,
	/// Could not be measured for any other reason (timeout, lost output, unreadable repository).
	Unknown(String),
}

fn fs_side(path: &Path) -> Side {
	match std::fs::metadata(path) {
		Ok(m) => Side::Size(m.len()),
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => Side::Absent,
		Err(e) => Side::Unknown(e.to_string()),
	}
}

/// Absence comes from core's exit-code check (unborn HEAD, a root commit's parent, a conflicted
/// path's stage 0 and a missing path all read as None); any error is Unknown and stops the read.
fn blob_side(repo: &Path, rev: &str, path: &str) -> Side {
	match blob_size(repo, rev, path) {
		Ok(Some(n)) => Side::Size(n),
		Ok(None) => Side::Absent,
		Err(e) => Side::Unknown(e.to_string()),
	}
}

/// The largest side of the diff: the new side and the old side are both measured, since either
/// can be the big one. A side that does not exist (an added or deleted file) is skipped; a side
/// that cannot be measured stops the read, because an unknown side may be the big one. Nothing
/// is sent to git unless every existing side has a known size.
pub(super) fn largest_side(open: &OpenDiff) -> Result<u64, String> {
	let new_path = open.display.as_str();
	let old_path = open
		.paths
		.iter()
		.find(|p| p.as_str() != new_path)
		.map_or(new_path, String::as_str);
	let sides = match &open.source {
		DiffSource::Working => [
			fs_side(&open.repo.join(new_path.replace('/', "\\"))),
			blob_side(&open.repo, ":0", old_path),
		],
		DiffSource::Staged => [
			blob_side(&open.repo, ":0", new_path),
			blob_side(&open.repo, "HEAD", old_path),
		],
		DiffSource::Commit(hash) => [
			blob_side(&open.repo, hash, new_path),
			blob_side(&open.repo, &format!("{hash}^"), old_path),
		],
	};
	let mut max: Option<u64> = None;
	for side in &sides {
		match side {
			Side::Size(n) => max = Some(max.map_or(*n, |m| m.max(*n))),
			Side::Absent => {}
			Side::Unknown(why) => {
				return Err(format!(
					"A side's size could not be read, so the diff was not run: {why}"
				));
			}
		}
	}
	match max {
		Some(n) => Ok(n),
		None => Err("Neither side exists any more; nothing to diff.".to_string()),
	}
}
