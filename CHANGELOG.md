# Changelog

All notable changes to repose are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the version follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-11-02

The first release: a native Windows viewer for every Git repository under your roots, built
with eframe/egui (ADR-001, chosen from measured prototypes).

### Added

- Repository list: every repository, worktree and submodule under the configured roots, with
  branch, short hash and state badges, worktrees grouped under their main repository, a Find box
  that filters as you type (Escape clears it), type-ahead selection and a Refresh button. The
  header lists the selected repository's configured remotes under its path.
- Changes tab: Staged Changes and Changes groups with counts, folder paths and letter badges, as
  VS Code's Source Control view shows them. Stage, unstage, discard (a confirmation lists every
  path) and commit with a message, all through the installed `git`, so hooks run as they do in a
  terminal. A commit whose hooks run long can be cancelled. The discard confirmation takes
  keyboard focus on Cancel and returns it to the Discard button that opened it.
- Diff viewer: read-only side-by-side and inline diffs for working-tree, staged and commit
  changes, with line numbers, hunk separators, word-level highlights, syntax colouring
  (PowerShell included) and a breadcrumb naming the repository, folder and file.
- History tab: the current branch's commits newest first, an Outgoing changes group for commits
  not yet on the upstream, each commit expanding to the files it changed, and Load more. If the
  outgoing read fails, the tab says so and keeps the last known outgoing commits.
- Files tab: the repository's tree at HEAD or at a selected commit, any file opened read-only in
  the same highlighted viewer.
- Branches tab: local branches with the current one marked, each with its short hash, upstream
  and ahead/behind from saved refs. Read-only; nothing is switched, created, deleted or fetched.
- Live refresh: a file-system watcher on each root refreshes only the repository that changed,
  debounced so a build produces one refresh, with a 60-second backstop. Your selection is kept.
- External actions: open a folder in Explorer, a file in VS Code at the line, or a pwsh
  terminal in the folder; copy a path, hash or remote URL; open the remote's page.
- Dark theme from named tokens, Segoe UI for interface text and Cascadia Code for names, hashes
  and code. A bottom row shows the version, commit and build time.
- Command-line flags: `--select`, `--view`, `--file`, `--inline`, `--open`, `--scale`,
  `--config` and `--measure-startup`.
- Configuration in `%APPDATA%\repose\config.toml`, created with defaults on first run.
- The Find box and the commit message field have visible labels, which are also their
  accessible names.
- `THIRD-PARTY-NOTICES.md` ships beside the executable with the licence texts of the Rust crates
  linked into it, the vendored PowerShell grammar, and the acknowledgements for the syntax
  definitions and themes compiled in through two-face.

### Security

- Every `git` command the installed app runs, stage, discard and commit included, disables the
  repository's `core.fsmonitor` setting, so reading or changing a repository never starts the
  file-system monitor program that repository's config names. Commit hooks still run, as they
  do in a terminal.
- A remote URL shown in the header never paints its userinfo, so a token or password stored in
  a remote URL stays off the screen. Copy still copies the URL exactly as configured. One
  malformed form is a known edge: a dotted username with a raw slash or hash in its password,
  which Git cannot use as a remote.
