# repose: decisions

## ADR-001: UI toolkit (accepted: eframe/egui)

Status: accepted 2026-11-01. Phillip chose eframe/egui (Room #30715) from the numbers and captures
below; the corrections in the next paragraph were posted to him at #30722 and he confirmed
"continue with egui" (#30724). Decided by the
`ui-toolkit-decision` record: two prototypes of the repository list, measured and screenshotted,
then Phillip's choice.

Corrections from Morticia's and Roger's reviews of the first table (02777d7): egui's three starts
ran during the last seconds of Alice's aidd gate and Tauri's on a quieter machine, so the toolkit
share column is partly load and the starts were warm, not cold; the Tauri working set sums seven
processes' working sets, which counts shared pages more than once, so it is an upper bound and the
172 against 335 ratio is not measured; and egui's 125% and 100% captures are 2304x1296 and
2880x1620 images (the window is 1920/scale points on a 150% display), not the same physical window
as its 150% capture. The smallest text in both prototypes is the 10.5 pt section label, which at
the 100% emulation is 10.5 px, under the spec's 11 px floor; the row meta line is 11.5 pt. Clause
3 of the record (captures at real Windows 100/125/150) is not met by this spike; the captures at
the display's own 150% are real, the other two are emulated, and Phillip chose with that stated.

Candidates: eframe/egui 0.37 and Tauri 2.9 (WebView2). Both prototypes list the 72 repositories
under `D:\applications` and `D:\applications.worktrees` with name, branch, short hash and grouped
worktrees, scan the roots synchronously before the first frame, and use the same palette and fonts.
Measured on the development machine (Windows 11, display at 150% scaling, AppliedDPI 144), release builds, each
start number the median of three consecutive warm starts (the executable and its files were
already cached) unless stated. Evidence: `.aidd/screenshots/toolkit-spike/`
(captures, measure JSON, prototype sources, scripts, SHA256SUMS).

| Measure                                          | eframe/egui                     | Tauri 2                                   |
| ------------------------------------------------ | ------------------------------- | ----------------------------------------- |
| Start to painted list (median of 3 warm starts)  | 1,559 ms (1,256 / 1,559 / 1,824) | 893 ms (877 / 893 / 896)                 |
| Of which the shared repository scan              | about 790 ms                    | about 740 ms                              |
| Toolkit share of start (start minus scan)        | about 770 ms                    | about 150 ms                              |
| Working set after 60 s                           | 172 MB, one process             | 26 MB app + 309 MB in 6 WebView2 processes = 335 MB |
| Executable size (release)                        | 18.8 MB                         | 7.9 MB, plus the OS WebView2 runtime      |
| Smallest text at 100% / 125% / 150%              | 10.5 / 13.1 / 15.75 px (computed from the 10.5 pt section label; the meta line is 11.5 pt) | the same, in CSS px |
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

Decision and reason: eframe/egui, Phillip #30715. The reason he was given and chose on: one
process against a browser engine in seven, with text that held up at the display's scaling; the
start-time and memory-ratio columns are indicative only, as corrected above. Consequences: the
spec's 300 ms paint budget is re-measured on a quiet machine once the real app paints before it
scans; the idle budget is restated as private working set of one process; the Tauri prototype is
deleted from scratch and the egui one is kept as the starting point for `visual-theme`.

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
