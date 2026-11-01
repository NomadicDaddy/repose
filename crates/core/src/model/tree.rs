//! One entry of a committed tree, as `git ls-tree -r -l` lists it: the path, the blob's size, and
//! whether it is a submodule rather than a file.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
	/// Slash-separated, relative to the repository root, as git prints it.
	pub path: String,
	/// The blob's size in bytes; 0 for a submodule, which has no blob.
	pub size: u64,
	/// A gitlink: the commit of a nested repository, not a file that can be opened.
	pub submodule: bool,
}

impl TreeEntry {
	pub fn file_name(&self) -> &str {
		self.path.rsplit('/').next().unwrap_or(&self.path)
	}

	/// The folder part, empty at the root.
	pub fn folder(&self) -> &str {
		self.path.rsplit_once('/').map_or("", |(f, _)| f)
	}
}
