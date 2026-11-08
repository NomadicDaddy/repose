# repose

A fast, lightweight desktop viewer for many Git repositories at once, written in Rust. It brings
the parts of VS Code's Source Control view that matter when you keep sixty or more repositories
on one machine: a list of every repository with its branch and state, the changes in the selected
one, a commit history with the files each commit touched (linear, or with every branch's lanes
when you turn Lanes on), read-only diffs with syntax
highlighting, and staging and committing from the same window. It is not an editor and it is not
Electron.

Status: 1.1.2 is the current release. Built with eframe/egui. See `CHANGELOG.md` for what it does. The specification is
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
  `cargo install cargo-audit --locked`. Running repose needs neither. The same two apply to the
  GitHub Actions gate, `.github/workflows/qc.yml`, which runs `scripts/qc.ps1 -SkipMeasure` on
  windows-latest for every push and pull request to main: the runner image has `pwsh` and
  rustup, and the workflow installs cargo-audit.

## Build, run and install

- After cloning, run `git config core.hooksPath scripts/smokestack/hooks` once. It turns on the
  leak guard: a pre-commit hook that refuses a commit whose staged changes look like a secret.
  The hooks are copies delivered by smokestack, and a clone does not set the path by itself.
- `pwsh scripts/qc.ps1` runs the full gate (format, clippy, tests, cargo audit, release build,
  the 300-line check, which counts every line including blank ones, and the notices check) and stages the release files in `target/dist`:
  `repose.exe`, `LICENSE`, `THIRD-PARTY-NOTICES.md` and `RUST-STD-COPYRIGHT.html`. It then prints
  the executable size, startup time and idle memory as `MEASURE` lines, which are reported
  against targets and never fail the gate (`-SkipMeasure` skips the startup and memory runs).
  When every step passed, the tree is clean and you are on `main`, it also refreshes a per-user
  copy installed by `install-user.ps1`, or says why it left that copy alone. Its last line is
  `QC_EXIT=<code>`.
- The gate remembers passes, never failures, in `target/gate-cache`. A step that passed with
  exactly the same inputs prints `CACHED` with the time of that pass instead of running again.
  This covers format, clippy, tests, the release build, the 300-line check, the notices check and
  the startup and idle measurements. The inputs are the files under `crates`, the manifests, the
  lockfile, the tool configuration and the gate scripts. They also include the Rust tool versions
  and the cargo environment variables, the Git version and a hash of the global and system Git
  configuration for the tests (the code under test runs Git with them), and the licence inputs
  for the notices. The release build's inputs also include the commit and whether tracked files are
  edited, because the build stamps both into the executable, so a new commit always rebuilds. A
  cached release build also needs `target/release/repose.exe` to still be the executable that
  pass produced. Reused measurements are tied to the executable's bytes and the
  measurement script, and each one says when it was measured. The advisory audit and the release
  staging always run. `-NoCache` runs every step anyway. A step whose command cannot start (cargo
  missing from `PATH`, say) fails, like one that exits non-zero. `scripts/gate-cache-test.ps1`,
  run by every gate, checks the cache's hits and misses and that failure.
- A full uncached gate includes the idle memory run's 60-second sample. Give it at least 600
  seconds before calling it stuck. How long a run takes depends on the machine and on what is
  cached, so no timing here is a guarantee. On the development machine on 2026-11-08, the test
  step took 54 seconds on a warm build. Before the log tests built their 101 commits with one
  `git fast-import`, it took about 105. A command limit for `cargo test --workspace` should still
  sit well above a minute, and higher when the tests have to compile.
- `pwsh scripts/install-user.ps1` copies the staged files to `%LOCALAPPDATA%\Programs\repose` and
  adds that folder to your user PATH; `-Check` reports whether the installed copy is current.
- `repose` lists the repositories under the roots in `%APPDATA%\repose\config.toml` (created with
  defaults on first run), within its configured depth (3 folders by default) and skipping its
  exclude folders. Submodules are counted on their parent rather than listed as rows.
- The window follows the Windows app mode (Settings > Personalization > Colors), light or dark,
  and switches when you change it. To keep one theme, set `theme = "dark"` or `theme = "light"`
  in config.toml. The default is `theme = "system"`.
- Ahead and behind counts come from the refs already on disk. repose reaches the network only
  when you click Fetch, in a repository row's menu or the History header. That fetch never asks
  for a sign-in. If the remote wants one, it fails under "Fetch failed" with git's message, and
  you sign in from a terminal. After a fetch, that repository's line says "Fetched HH:MM".

## Command-line flags

- `--config <path>`: read this config file instead of `%APPDATA%\repose\config.toml`.
- `--select <path>`: open on this repository. Without it, starting from inside a listed
  repository selects that one.
- `--view changes|history|files|branches`: open on this tab. Any other value, or none, is
  refused with a message naming the four, and repose exits with code 2.
- `--file <path>`: open this file's working-tree diff; `--inline` shows it inline.
- `--lanes`: start with branch lanes on in History, as the Lanes toggle would, for the session.
- `--open <path>`: open this file read-only as HEAD holds it.
- `--scale <factor>`: zoom on top of the desktop's scaling, from 0.5 to 4. Any other value is
  refused and repose exits with code 2.
- `--measure-startup [file]`: write the milliseconds to the first frame with repository rows into
  the file (default `repose-startup-ms.txt`), print them, then exit.
- `--measure-diff <file>`: once the open diff has loaded, write one tab-separated line to the
  file (`diff-loaded`, the repository, the path, the hunk count and the milliseconds since
  start), and keep running. The gate's idle memory sample uses it.
- `--measure-lanes <file>`: with `--select <repo> --view history`, turn lanes on and wait for the
  first page. Then take 60 idle frames to find the refresh interval, and turn the mouse wheel
  over History one row a frame, 50 rows down and 50 back up. Only frames in which the view
  moved are counted. The tab-separated file holds:
  - the first-page time, split into `git log` and layout;
  - the widest lane count;
  - how many wheel frames moved;
  - the frame intervals (from one `ui()` start to the next) and the CPU time inside `ui()`
    (p50, p95, max);
  - the missed frames, and whether the run passed.

  Any real key, button, wheel turn or pointer movement during the run ends it with a failed
  report. repose then exits, non-zero when the run failed or the file could not be written.
  The pass rule is in `crates/app/src/measure_lanes.rs`.

- `--help` or `-h`: print these flags and exit with code 0.

Refusals and the help are written to stderr. The release build is a windowed program with no
console, so a refused value is also shown in a small window. To read the help from a terminal,
redirect it, for example `repose --help 2>&1 | Out-Host` in PowerShell.

## Layout

- `docs/specs/2026-11-01-repose-v1/`: technical specification, task order and decisions.
- `CONTEXT.md`: the domain glossary.
- `.aidd/`: specification, assertions, structure, roadmap and feature records (not tracked).
