//! Facts git does not hold, asked of a host through its own CLI. `github` is the only module that
//! spawns `gh`, as `git::commands` is the only one that spawns git (A1).

pub mod github;
