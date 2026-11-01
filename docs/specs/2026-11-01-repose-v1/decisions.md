# repose: decisions

## ADR-001: UI toolkit (open)

Status: measured 2026-11-01, awaiting Phillip's choice. Decided by the `ui-toolkit-decision`
record: two prototypes of the repository list, measured and screenshotted, then Phillip's choice.

Candidates: eframe/egui 0.37 and Tauri 2.9 (WebView2). Both prototypes list the 72 repositories
under `D:\applications` and `D:\applications.worktrees` with name, branch, short hash and grouped
worktrees, scan the roots synchronously before the first frame, and use the same palette and fonts.
Measured on the development machine (Windows 11, display at 150% scaling, AppliedDPI 144), release builds, each
number the median of three cold starts unless stated. Evidence: `.aidd/screenshots/toolkit-spike/`
(captures, measure JSON, prototype sources, scripts, SHA256SUMS).

| Measure                                          | eframe/egui                     | Tauri 2                                   |
| ------------------------------------------------ | ------------------------------- | ----------------------------------------- |
| Cold start to painted list (median of 3)         | 1,559 ms (1,256 / 1,559 / 1,824) | 893 ms (877 / 893 / 896)                 |
| Of which the shared repository scan              | about 790 ms                    | about 740 ms                              |
| Toolkit share of start (start minus scan)        | about 770 ms                    | about 150 ms                              |
| Working set after 60 s                           | 172 MB, one process             | 26 MB app + 309 MB in 6 WebView2 processes = 335 MB |
| Executable size (release)                        | 18.8 MB                         | 7.9 MB, plus the OS WebView2 runtime      |
| Smallest text at 100% / 125% / 150%              | 11.5 / 14.4 / 17 px (computed from the 11.5 pt meta line) | the same           |
| Captures                                         | `egui-150-native.png`, `egui-125-emulated.png`, `egui-100-emulated.png` | `tauri-150-native.png`, `tauri-125-emulated.png`, `tauri-100-emulated.png` |

How the scales were produced: the display runs at 150%, so the `-native` captures are what the
screen shows. The 125% and 100% captures are emulated: egui by setting pixels per point to 1.25 and
1.0 on the same physical window, Tauri by CSS zoom 0.8333 and 0.6667 at the native DPI. A real
change of the Windows setting was not made.

What the numbers do not say: the scan time belongs to the prototype, not the toolkit, and the real
app scans asynchronously after the first paint; neither prototype met the spec's 300 ms paint or
150 MB idle budgets as written, so those budgets are revisited once the toolkit is chosen. egui's
toolkit share is mostly GPU device and shader setup on first run; Tauri's working set is spread
over processes the app does not own and that the OS updates with Edge.

Decision and reason: pending Phillip's choice (control posted 2026-11-01).

## ADR-002: git through the installed executable

Status: accepted 2026-11-01 (specification). All git reads and writes go through `git` with
machine-readable output; no libgit2 or gitoxide in v1.0. Reason: hooks, ignore rules, config and
long-path behaviour match the terminal and VS Code, which is the experience being replicated.
Cost: process spawns, bounded by a pool of 8 and the 3 s status budget. Revisit if the budget fails
on the development machine after incremental refresh is in place.

Amended 2026-11-01 after Morticia's review of e362aa7: every spawn sets `GIT_LITERAL_PATHSPECS=1`;
reads set `GIT_OPTIONAL_LOCKS=0` and `-c core.fsmonitor=false`; writes have no timeout; diffs pass
explicit `a/` and `b/` prefixes and both paths of a rename; a commit's files use
`--diff-merges=first-parent`. The parity claim now names what VS Code sets rather than assuming it.

## ADR-003: no network in v1.0

Status: accepted 2026-11-01 (Phillip, in session). Ahead and behind come from saved refs; the
status line says so. Fetch, pull and push are the `remote-sync` record in v1.1.
