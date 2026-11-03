# repose

A fast, lightweight desktop viewer for many Git repositories at once, written in Rust. It brings
the parts of VS Code's Source Control view that matter when you keep sixty or more repositories
on one machine: a list of every repository with its branch and state, the changes in the selected
one, a linear commit history with the files each commit touched, read-only diffs with syntax
highlighting, and staging and committing from the same window. It is not an editor and it is not
Electron.

Status: 1.0.0, built with eframe/egui. See `CHANGELOG.md` for what it does. The specification is
in `docs/specs/2026-11-01-repose-v1/` and the project's aidd records (local only, under `.aidd/`).

## Prerequisites

- Windows 11 x64 for the first release. Nothing in the design blocks Linux or macOS later.
- Git on `PATH`. repose drives the same `git` you already use, so hooks, ignore rules and config
  behave exactly as they do in the terminal.
- A Rust stable toolchain to build from source, plus Microsoft's C++ build tools (the MSVC linker
  and the Windows SDK, from the "Desktop development with C++" workload of Visual Studio or its
  Build Tools), which Rust's Windows target links with. No runtime is bundled or downloaded.

## Build, run and install

- `pwsh scripts/qc.ps1` runs the full gate (format, clippy, tests, cargo audit, release build,
  the 300-line check and the notices check) and stages `repose.exe` with
  `THIRD-PARTY-NOTICES.md` in `target/dist`. Its last line is `QC_EXIT=<code>`.
- `pwsh scripts/install-user.ps1` copies the staged pair to `%LOCALAPPDATA%\Programs\repose` and
  adds that folder to your user PATH; `-Check` reports whether the installed copy is current.
- `repose` opens on every repository under the roots in `%APPDATA%\repose\config.toml`, which is
  created with defaults on first run. `--select <path>` and `--view changes|history|files` open
  on a repository and tab.

## Layout

- `docs/specs/2026-11-01-repose-v1/`: technical specification, task order and decisions.
- `CONTEXT.md`: the domain glossary.
- `.aidd/`: specification, assertions, structure, roadmap and feature records (not tracked).
