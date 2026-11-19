//! repose core: finds repositories under configured roots and reads their state through the
//! installed `git`. No UI, and no network but the user's own Fetch, Pull, Push, Check visibility
//! and Suggest message (A1). See docs/specs/2026-11-01-repose-v1/technical-spec.md.

pub mod config;
pub mod discovery;
pub mod git;
pub mod host;
pub mod model;
pub mod pins;
pub mod settings;
pub mod suggest;
pub mod watch;

pub use config::Config;
pub use model::{BranchState, RepoKind, RepoSummary, StatusSummary};
