//! remote-sync clauses 2-4 through the real panes: Fetch from the repository row's menu and from
//! the History header queues one fetch; while it runs the row, the header and both controls say
//! so; a failure shows under "Fetch failed" with git's message and not in the commit box; a
//! success turns "Saved refs; no fetch" into "Fetched HH:MM". The harness's repository folder does
//! not exist, so the fetch a click starts fails at spawn and never reaches a network.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use eframe::egui::{self, Key};
use repose_core::config::Config;
use repose_core::git::remote::RemoteEntry;

use super::ui_harness::{click, frame, key, repo, text_at, texts};
use super::{AppState, View, WriteKind, WriteOutcome};

fn with_remote(name: &str, remotes: Vec<RemoteEntry>) -> (AppState, PathBuf) {
	let mut state = AppState::new(Config::default());
	let row = repo(name, SystemTime::now());
	let path = row.path.clone();
	state.requested_selection = Some(path.clone());
	assert!(state.merge_discovered(vec![row]), "the row is selected");
	state.remotes.insert(path.clone(), Ok(remotes));
	(state, path)
}

fn origin() -> Vec<RemoteEntry> {
	vec![RemoteEntry {
		name: "origin".into(),
		url: "https://example.invalid/team/thing.git".into(),
		push_url: None,
	}]
}

fn outcome(path: &Path, error: Option<&str>) -> WriteOutcome {
	WriteOutcome {
		repo: path.to_path_buf(),
		kind: WriteKind::Fetch,
		error: error.map(str::to_string),
		leftover_lock: None,
		notice: None,
		cancelled: false,
		unknown: false,
	}
}

fn has(shapes: &[egui::epaint::ClippedShape], fragment: &str) -> bool {
	texts(shapes).iter().any(|t| t.contains(fragment))
}

#[test]
fn the_row_menu_fetch_queues_one_fetch_and_the_row_and_header_show_it() {
	let ctx = egui::Context::default();
	let (mut state, path) = with_remote("fetch-menu", origin());
	let first = frame(&ctx, &mut state, Vec::new());
	assert!(
		has(&first, "Saved refs; no fetch"),
		"before any fetch: {:?}",
		texts(&first)
	);
	frame(&ctx, &mut state, key(Key::Menu));
	let open = frame(&ctx, &mut state, Vec::new());
	let at = text_at(&open, "Fetch").unwrap_or_else(|| panic!("no Fetch item: {:?}", texts(&open)));
	frame(&ctx, &mut state, click(at));
	assert!(state.writes.fetching(&path), "the click queued a fetch");
	assert!(
		matches!(
			state.writes.running.get(&path).map(|r| &r.kind),
			Some(WriteKind::Fetch)
		),
		"exactly the Fetch kind runs"
	);
	let busy = frame(&ctx, &mut state, Vec::new());
	assert!(has(&busy, "fetching…"), "the row shows the running fetch");
	assert!(
		has(&busy, "0123abc · Fetching…"),
		"the header's branch line says Fetching…: {:?}",
		texts(&busy)
	);
}

#[test]
fn the_history_header_fetch_button_queues_a_fetch_and_is_off_while_one_runs() {
	let ctx = egui::Context::default();
	let (mut state, path) = with_remote("fetch-history", origin());
	state.view = View::History;
	let shown = frame(&ctx, &mut state, Vec::new());
	let at =
		text_at(&shown, "Fetch").unwrap_or_else(|| panic!("no Fetch button: {:?}", texts(&shown)));
	frame(&ctx, &mut state, click(at));
	assert!(state.writes.fetching(&path), "the button queued a fetch");
	// A second click while it runs does nothing: the button is disabled.
	frame(&ctx, &mut state, click(at));
	let queued = state
		.writes
		.waiting
		.iter()
		.filter(|r| r.repo == path)
		.count();
	assert_eq!(queued, 0, "no second fetch is queued behind the first");
}

#[test]
fn a_failed_fetch_shows_gits_message_under_fetch_failed_and_not_in_the_commit_box() {
	let ctx = egui::Context::default();
	let (mut state, path) = with_remote("fetch-failed", origin());
	let message = "fatal: Cannot prompt because user interactivity has been disabled.";
	state.finish_write(outcome(&path, Some(message)), &ctx);
	let shapes = frame(&ctx, &mut state, Vec::new());
	assert!(has(&shapes, "Fetch failed"), "{:?}", texts(&shapes));
	assert!(has(&shapes, message), "git's own message is shown");
	assert!(
		!has(&shapes, "Change failed"),
		"the commit box does not repeat it"
	);
	assert!(
		has(&shapes, "0123abc · Saved refs; no fetch"),
		"a failed fetch claims no fetch time: {:?}",
		texts(&shapes)
	);
}

/// The status line laid out for `total` repositories, `fetched` of them fetched, and a filter that
/// keeps `matching` of them (0: no filter), with repose's own fonts and a local clock. Returns
/// the painted text and how many rows its galley takes.
fn status_rows(total: usize, fetched: usize, matching: usize, missing: bool) -> (String, usize) {
	let ctx = egui::Context::default();
	crate::theme::apply(&ctx);
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	let rows: Vec<_> = (0..total)
		.map(|i| {
			let name = if i < matching {
				format!("match-{i:03}")
			} else {
				format!("r-{i:03}")
			};
			repo(&name, t0)
		})
		.collect();
	for r in rows.iter().take(fetched) {
		state.fetched.insert(r.path.clone(), t0);
	}
	state.merge_discovered(rows);
	state.utc_offset = Some(-360);
	state.checked_at = Some(t0);
	if matching > 0 {
		state.filter = "match".to_string();
		state.filter_changed();
	}
	if missing {
		state.missing_roots = vec![PathBuf::from("D:/a/root/that/is/not/there/any/more")];
	}
	let shapes = frame(&ctx, &mut state, Vec::new());
	shapes
		.iter()
		.find_map(|s| match &s.shape {
			egui::epaint::Shape::Text(t) if t.galley.text().contains(" · Checked ") => {
				Some((t.galley.text().to_string(), t.galley.rows.len()))
			}
			_ => None,
		})
		.unwrap_or_else(|| panic!("no status line"))
}

/// Design-polish clause 3: after a fetch the list's status line stays on one line in the fixed
/// 300 px sidebar, measured on the painted galley with repose's own fonts. The sweep's case, then
/// three-digit counts (Morticia L-2) and a filtered line with them (L-3).
#[test]
fn the_status_line_stays_on_one_line_after_a_fetch() {
	// Control: the same measurement sees a wrap, here from a missing root's long note.
	let (text, rows) = status_rows(4, 1, 0, true);
	assert!(
		rows > 1,
		"the measurement sees a wrap: {text:?} took {rows} row(s)"
	);
	// Each case names its exact lead and fetch count (Roger #37558), so a mislabelled or unfiltered
	// line cannot pass the width check just by fitting.
	let cases = [
		(4, 1, 0, "4 repositories · ", " · 1 fetched, last "),
		(400, 123, 0, "400 repositories · ", " · 123 fetched, last "),
		(400, 123, 12, "12 of 400 match · ", " · 123 fetched, last "),
	];
	for (total, fetched, matching, lead, fetch) in cases {
		let (text, rows) = status_rows(total, fetched, matching, false);
		assert!(text.starts_with(lead), "it leads with {lead:?}: {text:?}");
		assert!(text.contains(fetch), "it says {fetch:?}: {text:?}");
		assert_eq!(
			rows, 1,
			"{total} repositories, {fetched} fetched, {matching} matching: {text:?} takes {rows} rows"
		);
	}
}

#[test]
fn a_successful_fetch_says_fetched_and_its_time() {
	let ctx = egui::Context::default();
	let (mut state, path) = with_remote("fetch-ok", origin());
	state.finish_write(outcome(&path, None), &ctx);
	let shapes = frame(&ctx, &mut state, Vec::new());
	let all = texts(&shapes);
	let header = all
		.iter()
		.find(|t| t.contains("0123abc · Fetched "))
		.unwrap_or_else(|| panic!("the header says Fetched HH:MM: {all:?}"));
	let time = header.rsplit("Fetched ").next().unwrap_or_default();
	assert!(
		time.len() >= 5 && time.as_bytes()[2] == b':',
		"HH:MM after Fetched: {header:?}"
	);
	assert!(
		has(&shapes, " · 1 fetched, last "),
		"the list line no longer says no fetch: {all:?}"
	);
	assert!(!has(&shapes, "0123abc · Saved refs; no fetch"));
	assert!(!has(&shapes, "Fetch failed"));
}

#[test]
fn with_no_remote_fetch_is_disabled_and_says_why() {
	let ctx = egui::Context::default();
	let (mut state, path) = with_remote("fetch-none", Vec::new());
	state.view = View::History;
	let shown = frame(&ctx, &mut state, Vec::new());
	let at = text_at(&shown, "Fetch").unwrap_or_else(|| panic!("{:?}", texts(&shown)));
	frame(&ctx, &mut state, click(at));
	assert!(
		!state.writes.fetching(&path),
		"a disabled Fetch queues nothing"
	);
	frame(&ctx, &mut state, key(Key::Menu));
	let open = frame(&ctx, &mut state, Vec::new());
	let item = text_at(&open, "Fetch").unwrap_or_else(|| panic!("{:?}", texts(&open)));
	frame(&ctx, &mut state, click(item));
	assert!(
		!state.writes.fetching(&path),
		"the menu's Fetch is disabled too"
	);
}
