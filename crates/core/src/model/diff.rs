//! A parsed unified diff of one file: the two paths, hunks with both line numbers on every line,
//! and the binary flag. Git's output is the source of lines; the app computes word highlights.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffSource {
	/// Working tree against the index.
	Working,
	/// Index against HEAD.
	Staged,
	/// A commit against its first parent.
	Commit(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
	Context,
	Added,
	Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
	pub kind: LineKind,
	pub old_no: Option<u32>,
	pub new_no: Option<u32>,
	pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
	pub old_start: u32,
	pub old_count: u32,
	pub new_start: u32,
	pub new_count: u32,
	/// The text after the second `@@`, git's guess at the enclosing function.
	pub heading: String,
	pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileDiff {
	/// None for a file that did not exist before (added).
	pub old_path: Option<String>,
	/// None for a file that no longer exists (deleted).
	pub new_path: Option<String>,
	pub binary: bool,
	pub hunks: Vec<Hunk>,
}

impl FileDiff {
	pub fn is_rename(&self) -> bool {
		matches!((&self.old_path, &self.new_path), (Some(a), Some(b)) if a != b)
	}

	/// The path to show: the new one, or the old one for a deletion.
	pub fn path(&self) -> &str {
		self.new_path
			.as_deref()
			.or(self.old_path.as_deref())
			.unwrap_or("")
	}

	pub fn added(&self) -> usize {
		self.count(LineKind::Added)
	}

	pub fn removed(&self) -> usize {
		self.count(LineKind::Removed)
	}

	fn count(&self, kind: LineKind) -> usize {
		self.hunks
			.iter()
			.flat_map(|h| h.lines.iter())
			.filter(|l| l.kind == kind)
			.count()
	}
}
