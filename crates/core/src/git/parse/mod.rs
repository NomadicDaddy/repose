//! Parsers for git's machine-readable output. Pure functions over bytes, tested with real output.

pub mod ls_tree;
pub mod porcelain;
pub mod unified;

pub use ls_tree::parse_ls_tree;
pub use porcelain::{Entry, EntryKind, Porcelain, parse_porcelain_v2};
pub use unified::parse_unified;
