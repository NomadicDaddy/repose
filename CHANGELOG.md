# Changelog

All notable changes to repose are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the version follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-11-04

The first release: a native Windows viewer for every Git repository under your roots, built
with eframe/egui (ADR-001, chosen from measured prototypes).

### Added

- Repository list: the repositories and worktrees under the roots, within the configured depth
  (3 folders by default) and skipping the configured exclude folders, with branch, short hash and
  state badges. A worktree is grouped under its main repository when that main is also found, and
  otherwise is its own row; submodules are counted on their parent rather than listed. The list
  has a Find box that filters as you type (Escape clears it), type-ahead selection and a Refresh
  button. The
  header lists the selected repository's configured remotes under its path, and shows ahead and
  behind against the upstream, or names an upstream that is gone.
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
  outgoing read fails, the tab says so and keeps the last known outgoing commits. A commit whose
  files can't be read says so in its own block and tries again when reopened.
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
- Command-line flags: `--select`, `--view` (changes, history, files or branches), `--file`,
  `--inline`, `--open`, `--scale`, `--config` and `--measure-startup`, listed in the README.
  `--select` accepts a relative path, a trailing separator and any letter case.
- Configuration in `%APPDATA%\repose\config.toml`, created with defaults on first run. Folder
  names in its exclude list match without regard to case.
- The Find box and the commit message field have visible labels, which are also their
  accessible names.
- repose is MIT-licensed; `LICENSE` ships beside the executable.
- `THIRD-PARTY-NOTICES.md` ships beside the executable with the licence texts of its Cargo
  dependency crates, the vendored PowerShell grammar, and the acknowledgements for the syntax
  definitions and themes compiled in through two-face. `RUST-STD-COPYRIGHT.html`, the Rust
  toolchain's own notice for the standard library linked into the executable, ships beside it.

### Security

- Every `git` command the installed app runs, stage, discard and commit included, disables the
  repository's `core.fsmonitor` setting, so reading or changing a repository never starts the
  file-system monitor program that repository's config names. Commit hooks still run, as they
  do in a terminal.
- The `git` commands repose runs to read a repository never fetch from a remote: with Git 2.45 or
  later, an object missing from a partial clone is reported as an error instead of being fetched,
  and `git remote` runs only as `remote --verbose`. Your own commit hooks still run as written and
  may use the network.
- A remote URL shown in the header does not paint its userinfo, so a token or password stored in
  an ordinary remote URL stays off the screen. Copy still copies the URL exactly as configured.
  Some malformed forms, none of which Git can use as a remote, can still show part of the
  userinfo: a dotted username with a raw slash or hash in its password, token-only userinfo with
  a raw question mark or slash (`https://tok?en@host/x` shows `https://tok`), and a password of
  digits followed by a slash.
- For ordinary remote URL forms, Open remote page hands the browser the remote's host and path
  only: the userinfo, any query and any fragment are left out, for scheme URLs and the
  `host:path` form alike. A URL with an at-sign after a raw question mark or hash, before its
  path begins, gets no page. The malformed forms with a raw slash in the userinfo can still pass
  part of it through.

### Known limitations

The first five were accepted for 1.0.0 by the owner (Phillip, #32993); the first three are on the
v1.1 roadmap. A sixth, the development machine's host name in early commits, was resolved before
publication by rewriting the history, so no published commit carries it.

- Changes, History and file-view rows show no keyboard focus outline.
- Painted rows and diff lines have no accessible names, so a screen reader cannot read them.
- The gate reruns every step on each run; there is no cache for unchanged checks.
- The ASSERTIONS audit could not run for this release; it is scheduled again for v1.1.
- `scripts/install-user.ps1` has been run only on the development machine; there is no
  clean-machine install record and no uninstall script yet.

Found later by the release's native check, and accepted for 1.0.0 by the owner (Phillip, in
session, recorded at #33446); the fix is on the v1.1 roadmap:

- When Open remote page is disabled because a URL origin has no page (for example
  `http://localhost?who=a@b`), its tooltip still says the origin is a local path.
