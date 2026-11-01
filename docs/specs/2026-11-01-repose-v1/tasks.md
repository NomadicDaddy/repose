# repose v1.0: task order

Each task names its feature record. Gates: `cargo fmt --check`, `cargo clippy --all-targets --
-D warnings`, `cargo test --workspace`, `cargo build --release`, all through `scripts/qc.ps1`
once it exists. Heavy steps (builds, the spike's measurements, screenshots) take a shared build slot under
the team's rules; the spec and record edits do not.

1. **Toolkit spike** (`ui-toolkit-decision`). Two throwaway prototypes of the repository list
   over the real roots, one eframe and one Tauri 2. Measure and screenshot per the record. Post the
   comparison; Phillip decides. Write ADR-001 in `decisions.md`. Delete the prototype that lost.
2. **Workspace and gate** (`quality-gate-and-release`, first half). `Cargo.toml` workspace,
   `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `scripts/qc.ps1` with fmt, clippy, test,
   release build and the line check. Budgets are added in step 12 when there is something to
   measure.
3. **Core: config and discovery** (`repository-discovery`). `config.rs`, `discovery/`, the
   temp-tree test with a repository, a worktree, a nested repository and a submodule.
4. **Core: git commands and status** (`repository-discovery`, `repository-list`). `commands.rs`
   with the allowed verbs and pool, `status.rs` with the porcelain v2 parser and fixtures, ahead
   and behind.
5. **Theme** (`visual-theme`). Tokens, fonts, base controls, a 1920x1080 capture compared with
   `look.jpg` at 100%, 125% and 150%.
6. **Repository list screen** (`repository-list`). Rows, badges, worktree groups, filter,
   Refresh, status line, keyboard.
7. **Changes screen** (`working-tree-changes`). Groups, badges, hover actions, click opens diff
   placeholder until step 10.
8. **Stage, discard, commit** (`stage-and-commit`). Confirmation dialog, commit box, hook failure
   display, tests S4 to S7.
9. **History** (`commit-history`). Log parser and fixtures, Outgoing group, expand to files, Load
   more.
10. **Diff viewer** (`diff-viewer`). Unified parser, side-by-side alignment, inline mode, word
    highlights, syntect, binary and size notices, breadcrumb.
11. **File browser** (`file-browser`). Tree at HEAD or commit, viewer reuse.
12. **Live refresh and budgets** (`live-refresh`, `quality-gate-and-release`, second half).
    Watcher, debounce, merge-by-path, backstop timer; `measure-startup.ps1` and the budget checks
    in `qc.ps1`; scenarios S1, S9, S10, S11.
13. **External actions** (`external-actions`). Platform module, context menus, config commands.
14. **Release readiness.** CHANGELOG, version, release table, design sweep of the planned surfaces
    in `.aidd/screen-map.md`, Roger and Morticia reviews. Nothing is pushed or published.

v1.1 records (`light-theme`, `branch-graph-lanes`, `remote-sync`, `settings-screen`) wait for
Phillip's go after the first release.
