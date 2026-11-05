# Changelog

All notable changes to repose are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the version follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - Unreleased

### Added

- Screen readers can read the painted rows. Repository, change and commit rows and file-view
  lines are buttons named from what they show, with their selected, open or current state. The
  worktree chevron is named by its action ("Expand 1 worktree"), and branch rows are list items.
- Diff lines, side-by-side pairs, hunk separators and group counts are readable text for
  assistive technology, without becoming Tab stops.
- The origin line in the repository header is a link to the repository's web page, the same
  page Open remote page opens. It shows the page as its tooltip and opens it with a click, or
  with Enter once Tab reaches it. Other remotes, and an origin with no web page, stay plain
  text.
- `--help` (or `-h`) lists every command-line flag and exits.
- Fetch, in a repository row's menu and the History header, brings the remote's commits in so
  the ahead and behind counts are current. It is the one action that reaches the network, and
  only on your click. It never asks for a sign-in: git's prompt and Git Credential Manager's
  window are both off, so a remote that wants one fails under "Fetch failed" with git's message.
  The row shows the fetch while it runs, and the repository's line then says "Fetched HH:MM".
- repose follows Windows' Animation effects setting (Settings > Accessibility > Visual
  effects). With it off, fades stop and scrolling jumps instead of gliding. The setting is read
  once, a couple of seconds after the first rows appear, so it never slows startup; there is no
  setting of repose's own for it.

### Fixed

- Commits whose hooks run for a long time no longer hold up everything else. Reads and writes
  now have separate limits of eight git processes each, so status, diffs, history, branches and
  files keep loading while up to eight commits wait on their hooks.
- Open in VS Code passes the path with backslashes even when a root in `config.toml` is
  written with forward slashes, as Explorer's paths already were.

- A `--view` value that names no pane, or none at all, is refused with a message naming the
  four panes and exit code 2, as a bad `--scale` is, instead of quietly opening Changes.
- Change rows, commit rows and file-view lines show the one-pixel accent outline when they
  hold keyboard focus, over their selected, open or current fill, like repository and branch
  rows. This resolves the first 1.0.0 known limitation.
- The 1.0.0 limitation that painted rows and diff lines had no accessible names is resolved by
  the additions above.
- A branch row's accessible name says "upstream gone" only when the row itself does, when
  neither the ahead nor the behind count is known, not when just one of them is missing.
- Copy branch name opens from the Menu key on the focused branch row, as every other row's
  menu does, not only from a secondary click.
- Closing the window with an unsaved commit message asks first. It names the repositories, with
  Keep editing (the default, also Escape) or Close without committing; drafts live only in
  memory, so the close used to drop them silently.
- Syntax colours read at 4.5:1 or better on every background they're drawn over, including
  added and removed lines and their word marks. A colour that falls short is lightened toward
  the text colour just enough. Comments, which fell to 1.83:1 on an added word mark, now read
  as a lighter grey.
- Screen readers announce the repository scan and status line, the commit box's progress and
  outcome ("Committed.", "Commit failed" and the like), and "Reading status…" as they change.
  The status line's announcement leaves out its clock, so the minute-by-minute check doesn't
  repeat it when nothing else changed.
- Open remote page works for a remote on a bracketed IPv6 host, such as
  `ssh://git@[2001:db8::1]:2222/repo.git` or `git@[2001:db8::1]:group/repo.git`. It opens
  `https://[2001:db8::1]/…` instead of a page cut at the address's first colon. A bracket that
  never closes gets no page.
- When Open remote page is unavailable for a URL origin repose refuses (an unsupported scheme or
  an unsafe form such as `http://localhost?who=a@b`), its tooltip now says so. It no longer
  claims the origin is a local path, which stays the reason only for a real path or `file://`
  origin. This resolves a 1.0.0 known limitation.
- THIRD-PARTY-NOTICES.md lists only the crates linked into the executable. It no longer lists
  13 that a disabled feature leaves out (egui_glow, the glutin crates and an image-decoding
  chain among them), so its crate count drops from 208 to 195.
- A file event whose path goes up and back down (`thing\..\thingamajig\x`) refreshes the
  repository the path really lands in, not the one its spelling starts with.

### Changed

- The gate's 300-line check counts every line, blank ones included, as the fleet's check does.
  Six files over that limit were split along existing seams, with no change in behaviour.
- The README and technical spec describe every gate step: the measurements are reported
  against targets and never fail the gate, `-SkipMeasure` skips the startup and memory runs,
  and a green run on a clean `main` refreshes an existing per-user install.
- The repository ignores a local `AGENTS.md`, as the other repositories in the workspace do.
- The gate's idle memory sample is taken with a diff open, as its target assumes. It runs in a
  throwaway fixture repository and config, never a user checkout. The new `--measure-diff
  <file>` flag reports when the diff has loaded.
- The gate no longer reruns a step that already passed with exactly the same inputs. It prints
  `CACHED` with the time of that pass. This covers format, clippy, tests, the release build, the
  file-length check, the notices check and the startup and idle measurements. Failures are never
  cached, the advisory audit and the release staging always run, and `-NoCache` forces every
  step. Reused measurements say when they were measured. A self-test of the cache runs in every
  gate.
- An internal tidy-up with no change in behaviour: the command-line flags are read from the same
  table `--help` prints, so a flag can't be added without its help line; the commit box, an
  expanded commit in History and the repository list are split into smaller pieces; the screens'
  text sizes and row heights are named theme values; and one helper turns a Git path into a file
  path. New tests drive the commit box's outcomes, an expanded commit's files, the worktree
  chevron and the filter's worktree rows through the real window code.
- The technical spec describes the platform module and the theme as they ship.

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
