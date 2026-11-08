//! repose core: finds repositories under configured roots and reads their state through the
//! installed `git`. No UI, no network. See docs/specs/2026-11-01-repose-v1/technical-spec.md.

pub mod config;
pub mod discovery;
pub mod git;
pub mod model;
pub mod pins;
pub mod watch;

pub use config::Config;
pub use model::{BranchState, RepoKind, RepoSummary, StatusSummary};
