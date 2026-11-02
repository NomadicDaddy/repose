//! One local branch as the Branches tab lists it: read-only, from saved refs, never fetched.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
	pub name: String,
	pub short_hash: String,
	/// The upstream's short name, when one is configured; a configured upstream whose ref is
	/// gone keeps its name and has no counts.
	pub upstream: Option<String>,
	pub ahead: Option<u32>,
	pub behind: Option<u32>,
	/// HEAD names this branch.
	pub current: bool,
}

/// The branches of one repository and what HEAD is doing, for the tab's first row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branches {
	pub head: HeadState,
	/// The current branch first, then the rest by name.
	pub list: Vec<Branch>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadState {
	/// HEAD is on the named branch, which is in the list and marked current.
	OnBranch(String),
	/// HEAD points at a commit, not a branch.
	Detached(String),
	/// A repository with no commit yet.
	Unborn,
}
