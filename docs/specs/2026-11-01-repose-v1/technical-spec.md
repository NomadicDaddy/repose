# repose v1.0: technical specification

Written 2026-11-01 from Phillip's brief and his answers. Feature records under `.aidd/features/`
carry the acceptance clauses; this document holds the architecture and the decisions the records
share. Decisions, open and resolved, are listed at the end and in `decisions.md`.

## Overview

repose is a Windows desktop app that shows many Git repositories at once with the parts of VS
Code's Source Control view Phillip actually uses: the repository list with branch and state, the
selected repository's Staged and Changes groups, a linear history whose commits expand to their
files, read-only diffs and files with highlighting, and a commit box. Scope settled in session:

| Question           | Answer                                                                 |
| ------------------ | ---------------------------------------------------------------------- |
| Changes            | View, stage, unstage, discard and commit. No push or pull (ADR-003).   |
| Files              | Read-only diffs and a read-only file viewer. Editing is VS Code's.     |
| Discovery          | Scan configured roots; worktrees grouped under their main repository.  |
| History            | Linear log with expandable commits and an Outgoing changes group.      |
| Refresh            | File-system watcher with debounce, plus Refresh and a backstop timer.  |
| Actions            | Explorer, VS Code, terminal, copy path, hash and remote URL.           |
| Platform           | Windows 11 first; nothing that blocks Linux or macOS.                  |
| Look               | Dark only, the `look.jpg` palette.                                     |
| Toolkit            | eframe/egui, chosen by a measured comparison (ADR-001, accepted).      |

## Architecture

```text
crates/core   discovery, git, watch, config, model     no UI dependency, fixture tests
crates/app    window, screens, state, theme, platform  the only consumer of core
```

Plain data crosses the boundary: `RepoSummary`, `Changes`, `Commit`, `FileDiff`. `core` exposes
synchronous functions returning those; `app` runs them on worker threads, receives the results
over a channel in its event loop and merges them into `state` by repository path, which is what
keeps selection and scroll stable across refreshes.

### Git backend: the installed `git`

Decision: every git read and write goes through the installed `git` executable, through one module
(`core::git::commands`) with an allowed-verb list: status, rev-parse, log, show, diff, ls-tree,
cat-file, add, restore, clean, commit, rm --cached, symbolic-ref, remote (only as
`remote --verbose`; extra arguments are refused), for-each-ref, and fetch (only as
`fetch --quiet --no-recurse-submodules --no-auto-maintenance`, on the user's Fetch, with
`GCM_INTERACTIVE=never`, `-c credential.interactive=false`, `-c protocol.ext.allow=never`,
`-c core.askPass=` and no askpass variables, beside `GIT_TERMINAL_PROMPT=0`; extra arguments are
refused).
Fetch runs in the per-repository write queue, so it never overlaps a stage or commit there. Every call runs with
`-c core.fsmonitor=false`, writes included, so a repository's own config cannot name a program for
repose to run, and with `GIT_NO_LAZY_FETCH=1`, so a partial clone's missing object is an error and
never a fetch from its remote. Machine-readable forms only:

- Status: `git status --porcelain=v2 -z --branch --untracked-files=all` (branch, upstream, ahead
  and behind from the `branch.ab` line, entries with rename scores and conflict states in one
  call). Reads run with `GIT_OPTIONAL_LOCKS=0` so a background status never rewrites the index or
  takes `index.lock` (the installed VS Code extension does the same).
- History: `git log -z --format=<fields with %x00 separators> --max-count=100 --skip=<n>` and
  `git log -1 -z --name-status --diff-merges=first-parent --format=` for a commit's files on
  expand, so a merge commit lists its first-parent changes instead of nothing.
- Outgoing: the ahead count from the status call's `branch.ab`; no upstream means no Outgoing
  group. No separate `rev-list` spawn.
- Branches: `git for-each-ref --format=<refname:short, objectname:short, upstream:short,
  upstream:track, HEAD with %00 separators> refs/heads` for the read-only Branches tab, with
  `git rev-parse --short HEAD` only when no branch is current, to name a detached HEAD. The
  ahead and behind counts are `upstream:track`, saved refs as everywhere else; no fetch.
- Remotes: `git remote --verbose` once per selected repository (saved config, no network), for
  the header's one line per configured remote, `<name> · <url>` with a differing push URL in
  parentheses, and for the menu's Copy remote URL and Open remote page, which use origin. No
  line when none is configured.
- Diff: `git diff --no-color --no-ext-diff --src-prefix=a/ --dst-prefix=b/ -U3 -- <paths>`
  (working tree vs index), `git diff --cached` (index vs HEAD) and
  `git show --format= --diff-merges=first-parent <hash> -- <paths>` (commit vs first parent),
  with `--find-renames`. For a renamed entry both the old and the new path are passed, from
  porcelain v2 for the working tree and index and from `--name-status` for a commit, so the diff
  is a rename and not a new file. The explicit prefixes keep `diff.noprefix` in a user's config
  from changing what the parser reads. Word-level highlighting is computed in the app with the
  `similar` crate on changed line pairs; git's output stays the source of lines.
- Files: `git ls-tree -r -z <rev>` and `git cat-file -p <rev>:<path>`.
- Writes: `git add --`, `git restore --staged --`, `git restore --`, `git clean -f --`,
  `git rm --cached` (unstaging in a repository with no commit yet) and `git commit -F <tempfile>`.
  Never `--no-verify`.

Why not libgit2 or gitoxide: parity. Hooks under `core.hooksPath`, ignore rules, `core.longpaths`,
credential helpers and config all behave as they do in the terminal and in VS Code, which is the
view being replicated, and the Summon team's `.githooks` deliveries run unchanged. The cost is
process spawns: about 75 at startup. A bounded pool of 8 keeps that under the 3 s budget on the
development machine (VS Code spawns the same status command, with `GIT_OPTIONAL_LOCKS=0`, which
repose copies). If a
later measurement shows the pool is the limit, the first optimisation is per-repository
incremental refresh, not a library.

Every spawn sets `-c core.quotePath=false`, `GIT_TERMINAL_PROMPT=0`, `GIT_LITERAL_PATHSPECS=1`,
`LC_ALL=C` and a working directory of the repository. `GIT_LITERAL_PATHSPECS=1` is what makes "the
paths passed to git are exactly the paths listed" true: without it `[` in a file name is a glob,
and `git restore -- '[1].txt'` also restores `1.txt` (Morticia, 2026-11-01). Reads carry a 10 s
timeout, after which the row shows an error badge with the stderr tail. Writes (`add`, `restore`,
`clean`, `commit`) have no timeout, because a hooked commit in this fleet can run format, leak
guard and tests for minutes; the row shows a running state with a Cancel that kills the process
only when the user asks. Cancel kills the whole process tree with `taskkill /PID <git> /T /F`.
When that succeeds a hook's children do not outlive it; when it fails the app says so, and the
hook's children may still be running. If an
index lock remains afterwards the app reports it with its path and never deletes it. The path
comes from `git rev-parse --git-path index.lock`, because a worktree's lock lives under the main
repository's `.git/worktrees/<name>/`, not under the worktree's own folder. Writes
run one at a time per repository: a second stage, discard or commit queues behind the first. Every
spawn has stdin set to null, so a hook that reads input gets end-of-file at once instead of waiting
on a terminal that is not there; a hook written to handle that carries on, and one that is not
exits with its own error.

### Discovery

Roots from `config.toml` are walked to depth 3 with pruning of `node_modules`, `target`, `dist`,
`.git` and the config's `exclude` names. A `.git` directory is a repository. A `.git` file is read:
`gitdir:` resolving under some `<main>/.git/worktrees/` makes it a worktree of `<main>`; any other
`gitdir:` makes it a repository. `.gitmodules` in a repository gives a submodule count; submodule
paths are not rows. `D:\applications.worktrees\<project>\<agent>\<task>` therefore lists under
`D:\applications\<project>` even though it is under a different root. Discovery runs once at
startup and on Refresh; status runs per repository and is what the watcher re-triggers.

### Watcher

One recursive `notify` watch per root (ReadDirectoryChangesW on Windows). Each event is mapped to
the owning repository by longest path prefix, with one exception: an event under
`<main>/.git/worktrees/<name>/` belongs to that worktree, whose index and HEAD live there rather
than under its own folder. Events are dropped if under an excluded directory or under `.git`
other than `HEAD`, `index`, `ORIG_HEAD`, `MERGE_HEAD`, `packed-refs`, `refs/`, `logs/HEAD` and
`worktrees/<name>/{HEAD,index,logs/HEAD}`, then debounced per repository at 250 ms (trailing
edge). A refresh re-runs status for
that repository, and the open Changes, diff or History view if it belongs to it. A 60 s timer
re-reads all statuses as a backstop. Watch overflow (the OS buffer filled) triggers one full
re-read and a status-line note.

### UI state

`app::state` holds the repository list, the selection, the open view and scroll offsets. Results
arrive on a channel tagged with repository path and a generation number; stale results are dropped.
A refresh replaces rows by path and keeps the selected path even if its position changed.

### Theme

Tokens from `look.jpg`, as values the Bolt sketch used for the same look:

| Token         | Value     | Use                                  |
| ------------- | --------- | ------------------------------------ |
| bg-app        | `#101518` | window                               |
| bg-panel      | `#161e22` | panes                                |
| bg-raised     | `#182328` | rows, cards                          |
| bg-hover      | `#233137` | hover                                |
| line / soft   | `#2b3539` / `#263036` | separators               |
| text / bright | `#dce4e4` / `#e0ebe8` | body, names              |
| muted         | `#97a8ad` | secondary text                       |
| accent        | `#87d6b2` | selection, section labels, links     |
| accent-bg / accent-line | `#283c3a` / `#608b7b` | selected row     |
| info          | `#85a3d0` | hashes                               |
| warn          | `#e3be7b` | M badge, ahead                       |
| danger        | `#f07878` | D badge, conflicts, errors           |
| added         | `#87d6b2` on `#1b2f28` | diff additions          |
| removed       | `#f07878` on `#3a2426` | diff removals           |

Fonts: Segoe UI (UI), Cascadia Code with Consolas fallback (names, hashes, code). Base size 13 px
at 100% scaling; the smallest text is 11 px. These are the starting values; the first screen is
captured and compared with `look.jpg` before the tokens are called done.

Sizes are tokens too, in `crates/app/src/theme/tokens.rs`, and no screen draws a size literal:
text 13 base, 11 small, 17 for the sidebar and dialog titles, 18 detail title, 12 metadata,
12 monospace, and 14 and 11.5 for a repository row's name and branch line; rows 46 for a
repository, 32 for a commit, 32 for a branch, 30 for a changed file and 20 for a diff or file line,
each its own token.

Motion follows Windows' Animation effects setting (Settings > Accessibility > Visual effects),
read once at launch off the UI thread. When it is off, egui's fades stop (`animation_time` 0) and
scrolling jumps instead of gliding. There is no repose setting for it.

### Platform module

`crates/app/src/platform/mod.rs` defines a `Platform` trait with one Windows implementation
(`platform/windows.rs`, chosen by `cfg(windows)`; other targets get one that returns an error for
each action):

- `open_folder(dir)`: `explorer.exe <dir>`.
- `reveal_file(file)`: `explorer.exe /select,"<file>"`, or the folder when the file is gone.
- `open_in_editor(command, target, line: Option<usize>)`: `<command> --goto <target>:<line>` when
  a line is given, otherwise `<command> <target>` (a folder, or a file with no line); a command
  with no extension that is not found is retried as its `.cmd`, and a missing editor is a notice
  naming the command.
- `open_terminal(command, dir)`: `<command> -d <dir> pwsh -NoLogo` (default `wt`), else an
  interactive pwsh in `<dir>`, started by a hidden pwsh through `Start-Process` so the new console
  gets its own input.

Every path handed to Explorer or the editor has its slashes turned to backslashes. Beside the
trait, the module has two free functions: `utc_offset_minutes()`, read from `w32tm /tz` and
re-read at most once a minute, and `git_worktree_path(repo, git_path)`, which turns a Git path
into the working-tree file. The clipboard and the browser go through egui's context
(`copy_text`, `open_url`), not the trait. The config file's location is not a platform method:
core's `config::default_path` chooses it (see Config). The editor and terminal commands are
configurable in `config.toml`.

### Config

`%APPDATA%\repose\config.toml`, created on first run:

```toml
roots = ["D:\\applications", "D:\\applications.worktrees", "D:\\bbs", "D:\\amiga", "D:\\infra", "D:\\scripts"]
exclude = ["node_modules", "target", "dist", ".prevers"]
depth = 3
[commands]
editor = "code"
terminal = "wt"
```

`--config <path>` overrides the location. There is no settings screen in v1.0 (record
`settings-screen`, v1.1).

### Budgets

Startup under 300 ms to a painted list, all statuses under 3 s, one refresh under 200 ms, idle
under 150 MB, executable under 25 MB, smooth scroll at 5,000 diff lines. `scripts/qc.ps1` measures
startup, the idle working set and the executable size on a release build and reports them; the
budgets are targets, not gates (Phillip, 2026-11-01).

## UI toolkit: eframe/egui (ADR-001, accepted 2026-11-01)

Resolved: Phillip chose eframe/egui from the measured comparison below (`decisions.md`, ADR-001).
The comparison is kept as it was planned and run.

Candidates: eframe/egui (immediate mode, wgpu, one executable) and Tauri 2 (Rust core, HTML/CSS in
the OS WebView2). iced and Slint were considered and set aside for the comparison: iced for the
same text-rendering question as egui with a smaller ecosystem, Slint for its licence and markup
model. The spike (`ui-toolkit-decision`) builds the repository list screen in both against the real
roots and records, for each: cold start to paint, working set after 60 s, executable size, a
1920x1080 screenshot at 100%, 125% and 150% scaling, and the smallest rendered text in physical
pixels. Phillip chooses from the screenshots and numbers; `decisions.md` records ADR-001 and the
`crates/app` layout is confirmed or adjusted after it.

## Quality gate

`scripts/qc.ps1`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test --workspace`, `cargo audit` (through `scripts/audit.ps1`), `cargo build --release`,
staging the release files in `target/dist` (`scripts/stage-release.ps1`), a 300 physical-line file check (blank lines count)
over `crates/**/*.rs`, the third-party notices check (`scripts/third-party-notices.ps1 -Check`),
then the measurements: the executable size and, unless `-SkipMeasure` is given, the median of
three starts to a painted list and the private working set after 60 s idle with a diff open, in
a disposable fixture repository the script creates and removes, sampled once `--measure-diff`
reports the diff loaded (`scripts/measure-startup.ps1`). They are printed as `MEASURE` lines and never change `QC_EXIT`
(assertions A8 and A9; Phillip, 2026-11-01). Last, when every step passed and a per-user copy
exists, the gate refreshes that copy from `target/dist`, but only for a clean tree on `main`;
otherwise it prints why the copy was kept. That refresh never changes the exit code either.

The gate caches passes only, never failures (`scripts/gate-cache.ps1`, records in
`target/gate-cache`). The format, clippy, test, release-build, file-length and notices steps
are skipped as `CACHED` when the key of their complete inputs matches an earlier pass. That key
covers each input file's path and SHA-256, plus the tool versions and cargo environment, the Git
version for the tests, and the licence inputs and standard-library notice hash for the notices.
The release build's key also carries `git rev-parse HEAD` and the dirty bit `build.rs` stamps
(tracked files differing from the commit), so a commit of the same bytes rebuilds and the
executable names its own commit. A cached release build also requires the executable it
produced to be unchanged. The startup and
idle measurements are reused only for the same executable, measurement script and conditions,
only when both were measured, and with their original time stated. `cargo audit` and the
staging always run, `-NoCache` forces every step, and `scripts/gate-cache-test.ps1` runs first
in every gate. The release table cites its log. These are build-time tools: PowerShell 7 and cargo-audit are prerequisites of the gate, not
of running repose. Hooks are a smokestack delivery and
Alice's; until one exists for a Rust repository, commits of source wait for it or run the gate by
hand and say so.

## Open decisions

1. Resolved: ADR-001 toolkit is eframe/egui (`decisions.md`).
2. Whether `D:\applications.worktrees` is a root or found through each main repository's
   `.git/worktrees/`; the spec says a root, because an agent's worktree can be registered against
   a main repository outside the scanned roots. Revisit if the list shows orphans.
3. Resolved: highlighting is `syntect` with the `fancy-regex` feature (no C dependency) and the
   two-face syntax and theme set, as Cargo.toml pins them. A 5,000-line diff takes 6 to 11 s to
   appear (recorded with the row-layout performance finding for v1.1); `tree-sitter` was not
   needed.
