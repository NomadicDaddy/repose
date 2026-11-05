# repose

A fast, lightweight desktop viewer for many Git repositories at once, written in Rust. It brings
the parts of VS Code's Source Control view that matter when you keep sixty or more repositories
on one machine: a list of every repository with its branch and state, the changes in the selected
one, a linear commit history with the files each commit touched, read-only diffs with syntax
highlighting, and staging and committing from the same window. It is not an editor and it is not
Electron.

Status: 1.0.0 is the current release; `main` is 1.1.0, unreleased. Built with eframe/egui. See `CHANGELOG.md` for what it does. The specification is
in `docs/specs/2026-11-01-repose-v1/` and the project's aidd records (local only, under `.aidd/`).

## Prerequisites

- Windows 11 x64 for the first release. Nothing in the design blocks Linux or macOS later.
- Git 2.45 or later on `PATH`. repose drives the same `git` you already use, so hooks, ignore
  rules and config behave exactly as they do in the terminal. repose sets `GIT_NO_LAZY_FETCH` so
  a partial clone never fetches missing objects; a Git build without support for that variable
  would. 2.45 is the supported minimum, and repose is tested with 2.56.
- A Rust stable toolchain to build from source, plus Microsoft's C++ build tools (the MSVC linker
  and the Windows SDK, from the "Desktop development with C++" workload of Visual Studio or its
  Build Tools), which Rust's Windows target links with. No runtime is bundled or downloaded.
- For the gate scripts only: PowerShell 7 (`pwsh`), and cargo-audit installed per user with
  `cargo install cargo-audit --locked`. Running repose needs neither.

## Build, run and install

- `pwsh scripts/qc.ps1` runs the full gate (format, clippy, tests, cargo audit, release build,
  the 300-line check, which counts every line including blank ones, and the notices check) and stages the release files in `target/dist`:
  `repose.exe`, `LICENSE`, `THIRD-PARTY-NOTICES.md` and `RUST-STD-COPYRIGHT.html`. It then prints
  the executable size, startup time and idle memory as `MEASURE` lines, which are reported
  against targets and never fail the gate (`-SkipMeasure` skips the startup and memory runs).
  When every step passed, the tree is clean and you are on `main`, it also refreshes a per-user
  copy installed by `install-user.ps1`, or says why it left that copy alone. Its last line is
  `QC_EXIT=<code>`.
- `pwsh scripts/install-user.ps1` copies the staged files to `%LOCALAPPDATA%\Programs\repose` and
  adds that folder to your user PATH; `-Check` reports whether the installed copy is current.
- `repose` lists the repositories under the roots in `%APPDATA%\repose\config.toml` (created with
  defaults on first run), within its configured depth (3 folders by default) and skipping its
  exclude folders. Submodules are counted on their parent rather than listed as rows.

## Command-line flags

- `--config <path>`: read this config file instead of `%APPDATA%\repose\config.toml`.
- `--select <path>`: open on this repository. Without it, starting from inside a listed
  repository selects that one.
- `--view changes|history|files|branches`: open on this tab. Any other value opens Changes.
- `--file <path>`: open this file's working-tree diff; `--inline` shows it inline.
- `--open <path>`: open this file read-only as HEAD holds it.
- `--scale <factor>`: zoom on top of the desktop's scaling, from 0.5 to 4. Any other value is
  refused and repose exits with code 2.
- `--measure-startup [file]`: write the milliseconds to the first frame with repository rows into
  the file (default `repose-startup-ms.txt`), print them, then exit.
- `--measure-diff <file>`: once the open diff has loaded, write one tab-separated line to the
  file (`diff-loaded`, the repository, the path, the hunk count and the milliseconds since
  start), and keep running. The gate's idle memory sample uses it.

There is no `--help` yet.

## Layout

- `docs/specs/2026-11-01-repose-v1/`: technical specification, task order and decisions.
- `CONTEXT.md`: the domain glossary.
- `.aidd/`: specification, assertions, structure, roadmap and feature records (not tracked).
