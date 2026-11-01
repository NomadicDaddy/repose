# repose

A fast, lightweight desktop viewer for many Git repositories at once, written in Rust. It brings
the parts of VS Code's Source Control view that matter when you keep sixty or more repositories
on one machine: a list of every repository with its branch and state, the changes in the selected
one, a linear commit history with the files each commit touched, read-only diffs with syntax
highlighting, and staging and committing from the same window. It is not an editor and it is not
Electron.

Status: specified, not built. The specification is in `docs/specs/2026-11-01-repose-v1/` and
the project's aidd records (local only, under `.aidd/`). The first piece of work is a toolkit
comparison; nothing else starts until that decision is recorded.

## Planned prerequisites

- Windows 11 x64 for the first release. Nothing in the design blocks Linux or macOS later.
- Git on `PATH`. repose drives the same `git` you already use, so hooks, ignore rules and config
  behave exactly as they do in the terminal.
- A Rust stable toolchain to build from source. No runtime is bundled or downloaded.

## Layout

- `docs/specs/2026-11-01-repose-v1/`: technical specification, task order and decisions.
- `CONTEXT.md`: the domain glossary.
- `.aidd/`: specification, assertions, structure, roadmap and feature records (not tracked).
