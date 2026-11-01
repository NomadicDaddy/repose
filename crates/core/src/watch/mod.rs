//! Live refresh from file-system events: which repository an event belongs to (`map`), whether
//! it deserves a refresh (`filter`), and how many refreshes a burst becomes (`debounce`). The
//! OS watcher itself lives in `watcher`; everything here is pure and tested without one.

pub mod debounce;
pub mod filter;
pub mod map;
pub mod watcher;

pub use debounce::{Debouncer, MAX_WAIT, WINDOW};
pub use filter::keep;
pub use map::{Mapped, RepoMap};
pub use watcher::{Refresh, Unwatched, Watch};
