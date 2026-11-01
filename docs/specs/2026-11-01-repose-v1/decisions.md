# repose: decisions

## ADR-001: UI toolkit (open)

Status: open, 2026-11-01. Decided by the `ui-toolkit-decision` record: two prototypes of the
repository list, measured and screenshotted, then Phillip's choice.

Candidates: eframe/egui and Tauri 2. Measurements to record here when the spike runs:

| Measure                                  | eframe | Tauri 2 |
| ---------------------------------------- | ------ | ------- |
| Cold start to painted list (median of 3) |        |         |
| Working set after 60 s                   |        |         |
| Executable size (release)                |        |         |
| Smallest text at 100% / 125% / 150%      |        |         |
| Screenshot paths                         |        |         |

Decision and reason: pending.

## ADR-002: git through the installed executable

Status: accepted 2026-11-01 (specification). All git reads and writes go through `git` with
machine-readable output; no libgit2 or gitoxide in v1.0. Reason: hooks, ignore rules, config and
long-path behaviour match the terminal and VS Code, which is the experience being replicated.
Cost: process spawns, bounded by a pool of 8 and the 3 s status budget. Revisit if the budget fails
on the development machine after incremental refresh is in place.

## ADR-003: no network in v1.0

Status: accepted 2026-11-01 (Phillip, in session). Ahead and behind come from saved refs; the
status line says so. Fetch, pull and push are the `remote-sync` record in v1.1.
