# repose v1.0: technical specification

Written 2026-11-01 from Phillip's brief and his answers. Feature records under `.aidd/features/`
carry the acceptance clauses; this document holds the architecture and the decisions the records
share. Decisions still open are listed at the end and in `decisions.md`.

## Overview

repose is a Windows desktop app that shows many Git repositories at once with the parts of VS
Code's Source Control view Phillip actually uses: the repository list with branch and state, the
selected repository's Staged and Changes groups, a linear history whose commits expand to their
files, read-only diffs and files with highlighting, and a commit box. Scope settled in session:

| Question           | Answer                                                                 |
| ------------------ | ---------------------------------------------------------------------- |
| Changes            | View, stage, unstage, discard and commit. No push, pull or fetch.      |
| Files              | Read-only diffs and a read-only file viewer. Editing is VS Code's.     |
| Discovery          | Scan configured roots; worktrees grouped under their main repository.  |
| History            | Linear log with expandable commits and an Outgoing changes group.      |
| Refresh            | File-system watcher with debounce, plus Refresh and a backstop timer.  |
| Actions            | Explorer, VS Code, terminal, copy path, hash and remote URL.           |
| Platform           | Windows 11 first; nothing that blocks Linux or macOS.                  |
| Look               | Dark only, the `look.jpg` palette.                                     |
| Toolkit            | Decided by a measured comparison, not assumed.                         |

## Architecture

```text
crates/core   discovery, git, watch, config, model     no UI dependency, fixture tests
crates/app    window, screens, state, theme, platform  the only consumer of core
```

Plain data crosses the boundary: `RepoSummary`, `Changes`, `Commit`, `DiffFile`. `core` exposes
async functions returning those; `app` drives them from its event loop and merges results into
`state` by repository path, which is what keeps selection and scroll stable across refreshes.

### Git backend: the installed `git`

Decision: every git read and write goes through the installed `git` executable, through one module
(`core::git::commands`) with an allowed-verb list. Machine-readable forms only:

- Status: `git status --porcelain=v2 -z --branch --untracked-files=all` (branch, upstream, ahead
  and behind from the `branch.ab` line, entries with rename scores and conflict states in one
  call). Reads run with `GIT_OPTIONAL_LOCKS=0` so a background status never rewrites the index or
  takes `index.lock` (the installed VS Code extension does the same), and with
  `-c core.fsmonitor=false` so a repository's own config cannot name a program for repose to run.
- History: `git log -z --format=<fields with %x00 separators> --max-count=100 --skip=<n>` and
  `git log -1 -z --name-status --diff-merges=first-parent --format=` for a commit's files on
  expand, so a merge commit lists its first-parent changes instead of nothing.
- Outgoing: the ahead count from the status call's `branch.ab`; no upstream means no Outgoing
  group. No separate `rev-list` spawn.
- Branches: `git for-each-ref --format=<refname:short, objectname:short, upstream:short,
  upstream:track, HEAD with %00 separators> refs/heads` for the read-only Branches tab, with
  `git rev-parse --short HEAD` only when no branch is current, to name a detached HEAD. The
  ahead and behind counts are `upstream:track`, saved refs as everywhere else; no fetch.
- Diff: `git diff --no-color --no-ext-diff --src-prefix=a/ --dst-prefix=b/ -U3 -- <paths>`
  (working tree vs index), `git diff --cached` (index vs HEAD) and
  `git show --format= --diff-merges=first-parent <hash> -- <paths>` (commit vs first parent),
  with `--find-renames`. For a renamed entry both the old and the new path are passed, from
  porcelain v2 for the working tree and index and from `--name-status` for a commit, so the diff
  is a rename and not a new file. The explicit prefixes keep `diff.noprefix` in a user's config
  from changing what the parser reads. Word-level highlighting is computed in the app with the
  `similar` crate on changed line pairs; git's output stays the source of lines.
- Files: `git ls-tree -r -z <rev>` and `git cat-file -p <rev>:<path>`.
- Writes: `git add --`, `git restore --staged --`, `git restore --`, `git clean -f --`, and
  `git commit -F <tempfile>`. Never `--no-verify`.

Why not libgit2 or gitoxide: parity. Hooks under `core.hooksPath`, ignore rules, `core.longpaths`,
credential helpers and config all behave as they do in the terminal and in VS Code, which is the
view being replicated, and the Summon team's `.githooks` deliveries run unchanged. The cost is
process spawns: about 75 at startup. A bounded pool of 8 keeps that under the 3 s budget on the development machine
(VS Code spawns the same status command, with `GIT_OPTIONAL_LOCKS=0`, which repose copies). If a
later measurement shows the pool is the limit, the first optimisation is per-repository
incremental refresh, not a library.

Every spawn sets `-c core.quotePath=false`, `GIT_TERMINAL_PROMPT=0`, `GIT_LITERAL_PATHSPECS=1`,
`LC_ALL=C` and a working directory of the repository. `GIT_LITERAL_PATHSPECS=1` is what makes "the
paths passed to git are exactly the paths listed" true: without it `[` in a file name is a glob,
and `git restore -- '[1].txt'` also restores `1.txt` (Morticia, 2026-11-01). Reads carry a 10 s
timeout, after which the row shows an error badge with the stderr tail. Writes (`add`, `restore`,
`clean`, `commit`) have no timeout, because a hooked commit in this fleet can run format, leak
guard and tests for minutes; the row shows a running state with a Cancel that kills the process
only when the user asks. Cancel kills the whole process tree (`taskkill /T` semantics through the
Windows job object the spawn is placed in), so a hook's children do not outlive it, and if an
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
| info          | `#809fce` | hashes                               |
| warn          | `#e3be7b` | M badge, ahead                       |
| danger        | `#f07878` | D badge, conflicts, errors           |
| added         | `#87d6b2` on `#1b2f28` | diff additions          |
| removed       | `#f07878` on `#3a2426` | diff removals           |

Fonts: Segoe UI (UI), Cascadia Code with Consolas fallback (names, hashes, code). Base size 13 px
at 100% scaling; the smallest text is 11 px. These are the starting values; the first screen is
captured and compared with `look.jpg` before the tokens are called done.

### Platform module

A `Platform` trait with one Windows implementation: `open_folder` (`explorer.exe /select,<path>`
for files, `explorer.exe <dir>` for folders), `open_in_editor` (`code.cmd --goto <file>:<line>`,
located on PATH, else a notice), `open_terminal` (`wt.exe -d <dir> pwsh`, else
`pwsh -NoExit -WorkingDirectory <dir>`), `copy_text`, `open_url` (`ShellExecuteW` open), and
`config_dir` (`%APPDATA%\repose`). Commands are configurable in `config.toml`.

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
the first four on a release build and fails above budget.

## UI toolkit: open, decided by measurement

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
`cargo test --workspace`, `cargo build --release`, a 300-line file check over `crates/**/*.rs`,
then the budget measurements. The release table cites its log. Hooks are a smokestack delivery and
Alice's; until one exists for a Rust repository, commits of source wait for it or run the gate by
hand and say so.

## Open decisions

1. ADR-001 toolkit (the spike).
2. Whether `D:\applications.worktrees` is a root or found through each main repository's
   `.git/worktrees/`; the spec says a root, because an agent's worktree can be registered against
   a main repository outside the scanned roots. Revisit if the list shows orphans.
3. Highlighting library: `syntect` with the `fancy-regex` feature (no C dependency) unless the
   spike shows it too slow for 5,000-line diffs; `tree-sitter` is the fallback.
