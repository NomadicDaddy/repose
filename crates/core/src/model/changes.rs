//! The selected repository's working tree, grouped the way the Changes view shows it.

use crate::git::parse::{Entry, EntryKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
	Modified,
	Added,
	Deleted,
	Renamed,
	Untracked,
	Conflicted,
	TypeChanged,
}

impl ChangeKind {
	/// The one-letter badge.
	pub fn letter(self) -> &'static str {
		match self {
			Self::Modified => "M",
			Self::Added => "A",
			Self::Deleted => "D",
			Self::Renamed => "R",
			Self::Untracked => "U",
			Self::Conflicted => "C",
			Self::TypeChanged => "T",
		}
	}

	fn from_code(code: u8) -> Option<Self> {
		match code {
			b'M' => Some(Self::Modified),
			b'A' => Some(Self::Added),
			b'D' => Some(Self::Deleted),
			b'R' | b'C' => Some(Self::Renamed),
			b'T' => Some(Self::TypeChanged),
			_ => None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
	pub path: String,
	pub kind: ChangeKind,
	/// For a rename, where the file came from.
	pub from: Option<String>,
}

impl FileChange {
	pub fn file_name(&self) -> &str {
		self.path.rsplit('/').next().unwrap_or(&self.path)
	}

	pub fn folder(&self) -> &str {
		match self.path.rfind('/') {
			Some(i) => &self.path[..i],
			None => "",
		}
	}
}

/// Staged (index against HEAD), unstaged (working tree against index), untracked and conflicted,
/// as git status reports them. A file can be in both staged and unstaged, as it is in git.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
	pub conflicts: Vec<FileChange>,
	pub staged: Vec<FileChange>,
	pub unstaged: Vec<FileChange>,
	pub untracked: Vec<FileChange>,
}

impl Changes {
	pub fn from_entries(entries: &[Entry]) -> Changes {
		let mut c = Changes::default();
		for e in entries {
			match &e.kind {
				EntryKind::Changed { xy } => {
					if let Some(kind) = ChangeKind::from_code(xy[0]) {
						c.staged.push(FileChange {
							path: e.path.clone(),
							kind,
							from: None,
						});
					}
					if let Some(kind) = ChangeKind::from_code(xy[1]) {
						c.unstaged.push(FileChange {
							path: e.path.clone(),
							kind,
							from: None,
						});
					}
				}
				EntryKind::Renamed { xy, from, .. } => {
					if xy[0] != b'.' {
						c.staged.push(FileChange {
							path: e.path.clone(),
							kind: ChangeKind::Renamed,
							from: Some(from.clone()),
						});
					}
					if let Some(kind) = ChangeKind::from_code(xy[1]) {
						c.unstaged.push(FileChange {
							path: e.path.clone(),
							kind,
							from: None,
						});
					}
				}
				EntryKind::Unmerged { .. } => c.conflicts.push(FileChange {
					path: e.path.clone(),
					kind: ChangeKind::Conflicted,
					from: None,
				}),
				EntryKind::Untracked => c.untracked.push(FileChange {
					path: e.path.clone(),
					kind: ChangeKind::Untracked,
					from: None,
				}),
				EntryKind::Ignored => {}
			}
		}
		for list in [
			&mut c.conflicts,
			&mut c.staged,
			&mut c.unstaged,
			&mut c.untracked,
		] {
			list.sort_by(|a, b| a.path.cmp(&b.path));
		}
		c
	}

	pub fn is_empty(&self) -> bool {
		self.conflicts.is_empty()
			&& self.staged.is_empty()
			&& self.unstaged.is_empty()
			&& self.untracked.is_empty()
	}

	/// The Changes group as the view shows it: unstaged edits plus untracked files.
	pub fn working(&self) -> impl Iterator<Item = &FileChange> {
		self.unstaged.iter().chain(self.untracked.iter())
	}
}
