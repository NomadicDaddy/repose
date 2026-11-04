//! The screens. Each file draws one pane from `AppState` and never owns state of its own.

mod a11y;
pub mod branches;
pub mod changes;
mod close_guard;
pub mod commit_box;
pub mod confirm;
mod crumb;
pub mod detail;
pub mod diff;
mod diff_draw;
pub mod file_view;
pub mod files;
pub mod footer;
pub mod history;
mod menu;
pub mod repo_list;
mod row;
