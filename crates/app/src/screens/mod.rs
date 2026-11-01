//! The screens. Each file draws one pane from `AppState` and never owns state of its own.

pub mod changes;
pub mod commit_box;
pub mod confirm;
pub mod detail;
pub mod diff;
mod diff_draw;
pub mod history;
pub mod repo_list;
mod row;
