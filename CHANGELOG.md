# Changelog

All notable changes to repose are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the version follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Settings, beside Refresh in the repository list, edits the roots, the folder names to skip,
  the scan depth, the theme and the editor and terminal commands without opening config.toml.
  A newly added root that does not exist, a name with a slash, or a command that is not a
  program name or absolute path is marked under its box, and Save waits until each is fixed. A
  root already in the config that is missing now (an unplugged drive) is only noted and kept.
  Save also waits while a New box holds text that was never added. Save rewrites only the
  settings that changed, keeping the other lines and their comments, pins and any key repose
  does not know. A changed setting keeps the comment lines above it, but its value is written
  fresh: a comment after the old value on the same line is dropped, and a changed list is
  written on one line. The changes then apply at once: the theme switches and new roots are scanned
  without a restart. A depth outside 1 to 8 that the file already held is kept unless changed.
  Cancel or Escape drops the edits; a click outside the screen does nothing.

### Fixed

- A worktree is listed under its repository when the root is written another way than git
  writes the folder, such as an 8.3 short name or a path through a junction. Before, it got a row
  of its own, and a change to its index refreshed the main repository's row instead of its own.
- With Lanes on, the current branch's line runs straight down its own lane to the first commit.
  Before, when a branch forked from an early commit that main later moved on from, main bent into
  that branch's lane on the way down, so the early commits read as the branch's. Now the branch
  keeps its own lane and joins main at the commit it forked from.
- A configured root that doesn't exist, such as an unplugged drive, is named once in the list's
  status line, as not found. Before, it was named a second time as "Not watching <path>: Input
  watch path is neither a file nor a directory", the file watcher's own error. A root that exists
  but can't be watched is still reported, in plain words: "Windows refused a watch on it; the
  60 s timer refreshes it".

### Development

- The status-line width test no longer depends on which folders exist on the machine running
  it. On the CI runner, where none of the default roots exist, every case carried "6 roots not
  found" and wrapped, so the first push gate on 1.2.0 failed (run 42903300911) while the gate on
  the development machine passed. The app itself is unchanged.
- The gate runs every test binary even after one fails (`cargo test --workspace
  --no-fail-fast`), so a red gate names every failing test at once; the step still fails. The
  cancel test now says why when a commit ends before its hook's child appears, instead of
  waiting out its 20 seconds, after the second CI run (42907194932) failed there with no reason.
- The tests' repositories carry their own committer identity. repose runs git with the
  machine's global config, and the CI runner has no identity there, so eight tests that commit
  were refused with "Author identity unknown" (run 42920036918). The same run's other two
  failures were the worktree fix above: the runner's temp folder is a short name.
- The shared gate scripts under `scripts/smokestack/` are at smokestack 0.7.3: the 0.7.2 and
  0.7.3 deliveries add the leak guard's self-test scripts there (its cases, its binary cases and
  its setup), and 0.7.3 makes the self-test check the refusal of an unreadable staged file on
  Linux git without depending on the machine's own git settings. repose's gate still runs its
  own `scripts/check-leak-guard.sh`; moving it to the delivered one is later work.

## [1.2.0] - 2026-11-08

### Added

- A light theme. The window follows the Windows app mode and switches when you change it; an
  open diff keeps its scroll and selection. `theme = "dark"` or `theme = "light"` in config.toml
  keeps one theme, and the default is `system`. Every text colour, placeholder text included,
  reads at 4.5:1 or more on every light background, and the selection and focus outlines at 3:1.
  Diffs and the file view use a light syntax theme in light mode.
- Branch lanes in History. A Lanes toggle in the History header, off by default, lists the
  commits of every local and remote branch and tag in topological order. It draws each line of
  history as a lane, with accent connectors where a merge brings a branch in and where a branch
  forked. A stash or notes ref is left out. Each commit shows its branch and tag names, which a
  screen reader hears too. An opened commit's details sit between rows, and the lanes break
  around them. The setting lasts until repose closes, and `--lanes` starts a session with it
  on. With Lanes off, History is the linear view of the current branch, as before.
- `--measure-lanes <file>` measures History with lanes on. It records the first page's time,
  split into `git log` and layout, then scrolls 100 rows with the mouse wheel and writes each
  frame's interval (from one frame's start to the next) and its CPU time, with a pass verdict.
  Any real input ends the run as failed. The branch-lanes record is measured with it.
- Each group in Changes (Merge Changes, Staged Changes and Changes) folds to its header with a
  click on the header or its triangle, and unfolds the same way. The count and Stage all or
  Unstage all stay on the header while it is folded. Each repository keeps its folds until repose
  closes.
- Pinned repositories. Pin or Unpin from a row's menu (right-click or the Menu key) keeps a
  repository at the top of the list, marked "pinned", in name order among the pinned ones. A
  worktree's menu pins its parent. Pins are saved as `pinned_repositories` in the config file
  in use, and only that key is rewritten, so other keys and comments are kept. A pin whose
  repository isn't found stays saved. If a save fails, the error shows and nothing changes.

### Fixed

- A remote URL too long for the header wraps only after a "/" or "\", as the path above it does,
  never inside a folder or host name. The origin link stays one link across its lines.
- File sizes of a gibibyte or more read in GB, so a 4 GiB file shows "4.0 GB", not "4096.0 MB".
  The Files pane and the "File too large" notices share the same tiers.
- A long file name in Changes, and a long branch name in Branches, is now cut with an ellipsis
  before the row's letter or pills. Before, it ran under them and was clipped at the row's edge.
  The full path or name shows on hover.
- The Find box's hint reads "name or path…" with an ellipsis, as the rest of the app does, not
  three full stops.
- A screen reader now hears each pane's loading text: Reading diff, Laying out, Reading tree,
  Reading file, Reading history, Reading branches, a commit's Reading files, and Load more's
  Loading. Before, only the status line and the commit messages were announced.
- Escape on the discard confirmation or the unsaved-message question now closes only that
  dialog. Before, the same key press also closed the diff or file open behind it.
- Reveal in Explorer on a file too large to diff, a changed file's Open, and Enter on the
  repository list now say when Windows couldn't open the folder. The reason shows under the
  repository header, as it already did from the right-click menu. Before, a failed click did
  nothing visible.
- When repose can't create its config file on first run, the error now says "could not write"
  and names the file. Before, it said "could not read". A failed pin save says the same.

### Development

- The Branches header's count uses the shared singular and plural rule, and a test pins its
  wording. The Cargo.toml comment on syntax colouring now cites that decision as resolved.
- The technical spec now describes what ships:
  - the Outgoing group comes from one extra `git log <upstream>..HEAD` read, not the ahead count;
  - git runs through two pools of eight, one for reads and one for writes;
  - the smokestack hooks live under `scripts/smokestack/hooks`;
  - History also has the lanes read.
- The gate now fails a step whose command can't start, such as cargo missing from `PATH`. Before,
  that step was recorded as a pass. The gate's self-test checks this.
- The test step's cache key now includes a hash of the machine's global and system Git
  configuration, which the tests run Git with. A change to either now reruns the tests.
- The log tests build their 101 commits with one `git fast-import` instead of 101 `git commit`
  calls. On a warm build the test step now takes about 54 seconds, down from about 105.
- A test checks that a screen reader can read every path in the discard confirmation, and that
  none of them is a Tab stop. The app already did this, so nothing changes in how it behaves.
- A GitHub Actions workflow, `.github/workflows/qc.yml`, runs the gate (`scripts/qc.ps1
  -SkipMeasure`) on windows-latest for every push to main and every pull request to main. It
  installs the toolchain `rust-toolchain.toml` names and cargo-audit, and keeps the gate's
  output as the `qc-log` artifact. It installs nothing on the runner beyond those.
- The UI tests draw every frame through one function in `state/ui_frame.rs`, instead of five
  test files each building their own, and share one row-rectangle finder and one
  three-changes fixture.
- A core test reads a branch whose remote-tracking branch is one commit ahead as 0 ahead and 1
  behind, from saved refs.
- An ignored test, `list_frame_times`, times a detail-pane frame for Changes, History and Files
  at 30, 1,000 and 5,000 rows. At 5,000 rows the median frame is 11.5 to 20.9 ms. On those
  numbers, row virtualisation moved to v2.0 instead of this release.
- The gate now tests the leak guard on every run, whether or not the hooks are installed.
  `scripts/check-leak-guard.sh` runs the delivered guard in a scratch repository: a clean
  staging passes; a token shape, a PEM key block and a private literal from a fixture pattern
  file are refused without being printed; and a configured pattern file that is missing refuses
  even clean content. It runs under Git's own bash, found beside git. The scratch repository
  has its own empty hooks folder, so hooks set in the global Git configuration don't run there.

### Known limitations

- Changes, History and Files lay out every row, not only the rows on screen. At 5,000 rows a
  frame takes 11.5 to 20.9 ms (median), and up to 30 ms in Files. Drawing only the visible rows
  is planned for v2.0.

### Accepted at this release

The release-readiness table for 1.2.0 had these lines Red or not checked, and Phillip accepted
each one:

- **C3, audits current** (#40476): of the 28 audits aidd counts as applicable, 24 are fresh.
  ASSERTIONS, AUDIT_METHODOLOGY, FEASIBILITY and SEVERITY_CLASSIFICATION have no report in the
  current audit-report folder, the same four as at 1.1.2.
- **C5, roadmap and priorities** (#40478): the records and roadmap entries reconcile, but `.aidd`
  is untracked, so no history shows whether a priority changed since 1.1.2.
- **F5, installer and first run** (#40479): `scripts/install-user.ps1`, unchanged since 1.1.2, has
  still been run only on the development machine; there is no clean-machine install record.

## [1.1.2] - 2026-11-07

### Development

- The gate's licence-notices check now runs again when `LICENSE` or the vendored PowerShell
  grammar's `LICENSE.txt` changes. Before, editing only those files kept the step cached, so a
  stale `THIRD-PARTY-NOTICES.md` could pass the gate. The gate's self-test now checks that the
  step's input list covers every file the notices generator reads from the repository.
- Tests pin what Enter and Space each do on every painted row that Tab reaches: a changed file
  opens its diff, a commit opens and closes, a commit's file opens that file's diff at the
  commit, a file-view line becomes the current line, and a repository becomes the selection
  without opening anything.

### Correction to 1.1.1's notes

- 1.1.1's known limitations said Enter doesn't open a changed file, commit or repository that
  Tab has reached. That was wrong: in 1.1.1 as released, Enter and Space already do what a click
  does on each of them, and the tests above pass on its unchanged app code.

### Known limitations

- Development: the other findings from 1.1.1's QC_PIPELINE draft are still open for a later
  release. A gate step whose command can't start is recorded as a pass; the test step's cache key
  leaves out the machine's global git configuration; the warm test suite takes about 105 s; and
  pushes to the public repository run no checks. None of them changes what repose does.

### Accepted at this release

The release-readiness table for 1.1.2 had these lines Red or not checked, and Phillip accepted
each one:

- **C3, audits current** (#39397): of the 28 audits aidd counts as applicable, 24 are fresh.
  ASSERTIONS, AUDIT_METHODOLOGY and SEVERITY_CLASSIFICATION have no report in the current
  audit-report folder, and FEASIBILITY's report says it could not run (no pending proposal or
  decision). 1.1.2 changes no app code.
- **C5, roadmap and priorities** (#39398): the records and roadmap entries reconcile, but `.aidd`
  is untracked, so no history shows whether a priority changed since 1.1.1.
- **F5, installer and first run** (#39399): `scripts/install-user.ps1` has still been run only on
  the development machine; there is no clean-machine install record.

## [1.1.1] - 2026-11-07

### Added

- With a filter typed in Find a repository, the list's status line says how many repositories
  match ("1 of 4 match"). When the filter hides the repository the right pane is showing, the
  pane says so under its name ("Hidden from the list by the filter").

### Fixed

- A diff whose first change is at the top of the file no longer opens with a
  "0 unchanged lines" separator; a separator shows only when it hides lines.
- Counts read correctly for one: "1 file", "1 line", "1 unchanged line", "1 path",
  "1 worktree", "1 repository".
- The list's status line stays on one line after a fetch; it now reads "1 fetched, last HH:MM"
  once something has been fetched, and "Saved refs; no fetch" before.
- The worktree chevron beside "1 worktree" is drawn to the text's size and sits on its line;
  it had been a small character above it.
- In an expanded commit, the change letter ("R", "M") sits on the same line as its file's path.
- The question shown when closing with an unsaved commit message has room around its text and
  buttons, and names each repository first, with its path below.
- A long repository path in the header wraps at a folder separator rather than inside a folder
  name.
- A remote written the scp way (`git@host:path`) no longer shows a `?` or `#` part in the header,
  where a token such as `?access_token=` could ride. A local path is still shown whole, since
  those characters are legal in a folder name.

### Development

- Commits are checked by a leak guard (a pre-commit hook delivered by smokestack) that refuses
  staged changes that look like secrets. After cloning, run
  `git config core.hooksPath scripts/smokestack/hooks` once to turn it on (README, "Build, run
  and install").
- `.records/` is no longer ignored: repose keeps its records under `.aidd/`.
- Tests lay out the list's status line with repose's own fonts and check its exact wording and
  that it stays on one line, at three-digit counts and with a filter typed.

### Known limitations

- Changed files, commits and repositories can be reached with Tab, but Enter doesn't open them;
  only a click does. This has been so since 1.0.0; a fix is planned for 1.1.2.
- Development: the gate's licence-notices check is cached on a key that leaves out `LICENSE`
  and the vendored PowerShell licence, so editing only those files can leave
  `THIRD-PARTY-NOTICES.md` stale on a passing gate. 1.1.1 changes neither file. It is to be
  fixed after 1.1.1.
- Development: the release audit run completed 7 of its 8 audits; QC_PIPELINE, which checks the
  gate itself, did not complete. Its draft findings are filed for after 1.1.1.

## [1.1.0] - 2026-11-05

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
  once, a couple of seconds after the first rows appear, so the read does not delay them; there
  is no setting of repose's own for it.

### Fixed

- A commit subject too long for its row in History is cut to one line with an ellipsis, and its
  tooltip shows it whole. It used to wrap inside the row and show stray fragments of its second
  line.
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
  text sizes and row heights are named theme values; the Changes and History panes share one
  colour map for a change's letter; and one helper turns a Git path into a file path. New tests drive the commit box's outcomes, an expanded commit's files, the worktree
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
