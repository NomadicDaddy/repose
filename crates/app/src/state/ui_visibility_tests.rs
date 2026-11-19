//! remote-visibility through the real panes: no badge until the user chooses Check visibility in
//! the row menu; an origin that is not on github.com comes back Unknown without running gh, so
//! nothing here reaches a network; gh's answer shows as the header's badge, is kept per remote URL
//! across Refresh and is never read again on its own; and with no origin the item is off.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui::{self, Key};
use repose_core::config::Config;
use repose_core::git::remote::RemoteEntry;
use repose_core::host::github::{NOT_GITHUB, Visibility};

use super::AppState;
use super::ui_harness::{click, frame, key, repo, text_at, texts};

const URL: &str = "https://example.invalid/team/thing.git";

fn with_origin(name: &str, url: Option<&str>) -> (AppState, PathBuf) {
	// No roots, so the Refresh one test runs walks nothing on this machine.
	let config = Config {
		roots: Vec::new(),
		..Config::default()
	};
	let mut state = AppState::new(config);
	let row = repo(name, SystemTime::now());
	let path = row.path.clone();
	state.requested_selection = Some(path.clone());
	assert!(state.merge_discovered(vec![row]), "the row is selected");
	let remotes = url.map_or_else(Vec::new, |url| {
		vec![RemoteEntry {
			name: "origin".into(),
			url: url.into(),
			push_url: None,
		}]
	});
	state.remotes.insert(path.clone(), Ok(remotes));
	(state, path)
}

fn has(shapes: &[egui::epaint::ClippedShape], fragment: &str) -> bool {
	texts(shapes).iter().any(|t| t.contains(fragment))
}

/// Opens the selected row's menu from the keyboard and clicks Check visibility.
fn choose_check(ctx: &egui::Context, state: &mut AppState) {
	frame(ctx, state, key(Key::Menu));
	let open = frame(ctx, state, Vec::new());
	let at = text_at(&open, "Check visibility")
		.unwrap_or_else(|| panic!("no Check visibility item: {:?}", texts(&open)));
	frame(ctx, state, click(at));
}

#[test]
fn check_visibility_shows_unknown_with_why_for_an_origin_not_on_github() {
	let ctx = egui::Context::default();
	let (mut state, _) = with_origin("visibility-unknown", Some(URL));
	let before = frame(&ctx, &mut state, Vec::new());
	assert!(
		!has(&before, "Visibility"),
		"no badge before the user asks: {:?}",
		texts(&before)
	);
	choose_check(&ctx, &mut state);
	let started = Instant::now();
	while !state.visibility.known.contains_key(URL) {
		assert!(
			started.elapsed() < Duration::from_secs(10),
			"the check answers"
		);
		std::thread::sleep(Duration::from_millis(10));
		state.pump(&ctx);
	}
	assert_eq!(
		state.visibility.known[URL].0,
		Visibility::Unknown(NOT_GITHUB.to_string()),
		"not github.com: Unknown, and gh never ran"
	);
	assert!(state.visibility.checking.is_empty());
	let after = frame(&ctx, &mut state, Vec::new());
	assert!(has(&after, "Visibility: Unknown"), "{:?}", texts(&after));
}

#[test]
fn an_answer_shows_as_the_badge_and_stays_across_refresh_without_a_new_read() {
	let ctx = egui::Context::default();
	let (mut state, path) = with_origin("visibility-public", Some(URL));
	state.visibility.checking.insert(URL.to_string());
	let waiting = frame(&ctx, &mut state, Vec::new());
	assert!(
		has(&waiting, "Checking visibility…"),
		"{:?}",
		texts(&waiting)
	);
	state.merge_visibility(URL.to_string(), Visibility::Public);
	let shown = frame(&ctx, &mut state, Vec::new());
	assert!(has(&shown, "Visibility: Public"), "{:?}", texts(&shown));
	state.start_scan(&ctx);
	assert!(
		state.visibility.known.contains_key(URL) && state.visibility.checking.is_empty(),
		"Refresh keeps the answer and asks gh nothing"
	);
	// Refresh re-reads the remotes; once origin is back, so is its badge.
	state.remotes.insert(
		path,
		Ok(vec![RemoteEntry {
			name: "origin".into(),
			url: URL.into(),
			push_url: None,
		}]),
	);
	state.merge_visibility(URL.to_string(), Visibility::Private);
	let again = frame(&ctx, &mut state, Vec::new());
	assert!(has(&again, "Visibility: Private"), "{:?}", texts(&again));
}

#[test]
fn with_no_origin_check_visibility_is_off() {
	let ctx = egui::Context::default();
	let (mut state, _) = with_origin("visibility-none", None);
	frame(&ctx, &mut state, Vec::new());
	choose_check(&ctx, &mut state);
	assert!(
		state.visibility.checking.is_empty() && state.visibility.known.is_empty(),
		"a disabled Check visibility asks nothing"
	);
}
