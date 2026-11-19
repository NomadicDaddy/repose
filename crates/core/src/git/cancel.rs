//! The user's Cancel for a running write.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A handle the user's Cancel sets; a write that was given it stops polling, kills git and the
/// whole tree git started, and returns `GitError::Cancelled`.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn cancel(&self) {
		self.0.store(true, Ordering::SeqCst);
	}

	pub fn is_cancelled(&self) -> bool {
		self.0.load(Ordering::SeqCst)
	}
}
