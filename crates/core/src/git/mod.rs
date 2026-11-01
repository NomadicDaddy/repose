//! Everything that talks to the installed `git`. `commands` is the only module that spawns it.

pub mod commands;
pub mod diff;
pub mod error;
pub mod log;
pub mod parse;
pub mod pool;
pub mod remote;
pub mod status;
pub mod tree;
pub mod write;

pub use commands::{Cancel, GitError, Verb, run};
pub use status::{read_status, read_status_into};
