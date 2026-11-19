//! remote-visibility through the real panes: no badge until the user chooses Check visibility in
//! the row menu; an origin that is not on github.com, or is refused as given, comes back Unknown
//! at the click without running gh, so nothing here reaches a network; no key holds a credential;
//! gh's answer shows as the header's badge, is kept per repository across Refresh and is never
//! read again on its own; and with no origin the item is off.

use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui::{self, Key};
use repose_core::config::Config;
use repose_core::git::remote::RemoteEntry;
use repose_core::host::github::{NOT_GITHUB, UNSAFE_NAME, Visibility};

use super::AppState;
use super::ui_harness::{click, frame, key, repo, text_at, texts};
use super::visibility::remote_key;

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

/// Every key the visibility state holds, answered or in flight.
fn held(state: &AppState) -> Vec<String> {
	let v = &state.visibility;
	v.known.keys().chain(v.checking.iter()).cloned().collect()
}

/// A credential-bearing origin (a token in its userinfo and in its query) is refused at the click,
/// with nothing started, and held only by its display_url, before and after Refresh clears the
/// remotes (Roger 5Q-3 on db5d258).
#[test]
fn check_visibility_shows_unknown_for_an_origin_not_on_github_and_keeps_no_credential() {
	let ctx = egui::Context::default();
	let secret_url =
		"https://user:secret-one@example.invalid/team/thing.git?access_token=secret-two";
	let (mut state, _) = with_origin("visibility-unknown", Some(secret_url));
	let before = frame(&ctx, &mut state, Vec::new());
	assert!(
		!has(&before, "Visibility"),
		"no badge before the user asks: {:?}",
		texts(&before)
	);
	choose_check(&ctx, &mut state);
	assert_eq!(
		state.visibility.answer(secret_url).map(|(v, _)| v),
		Some(&Visibility::Unknown(NOT_GITHUB.to_string())),
		"not github.com: Unknown at the click"
	);
	assert!(state.visibility.checking.is_empty(), "nothing was started");
	let after = frame(&ctx, &mut state, Vec::new());
	assert!(has(&after, "Visibility: Unknown"), "{:?}", texts(&after));
	state.start_scan(&ctx);
	let held = held(&state);
	assert_eq!(
		held,
		[format!("refused {URL}")],
		"only the credential-free form"
	);
	assert!(held.iter().all(|k| !k.contains("secret")), "{held:?}");
}

/// An scp origin with a query is refused as given, at the dispatch, though its display_url would
/// pass; it never shares an answer with the plain origin that names the same repository (Roger
/// 5Q-3 on b551265).
#[test]
fn an_scp_origin_with_a_query_is_refused_at_the_click_and_shares_no_answer() {
	let ctx = egui::Context::default();
	let with_query = "git@github.com:team/thing.git?access_token=secret";
	let (mut state, _) = with_origin("visibility-scp-query", Some(with_query));
	frame(&ctx, &mut state, Vec::new());
	choose_check(&ctx, &mut state);
	assert_eq!(
		state.visibility.answer(with_query).map(|(v, _)| v),
		Some(&Visibility::Unknown(UNSAFE_NAME.to_string()))
	);
	assert!(state.visibility.checking.is_empty(), "gh was never started");
	assert!(
		state
			.visibility
			.answer("git@github.com:team/thing.git")
			.is_none(),
		"the plain origin has no answer of its own yet"
	);
	assert!(held(&state).iter().all(|k| !k.contains("secret")));
}

/// A GitHub origin's answer, merged as gh's thread would send it (no gh runs here).
#[test]
fn an_answer_shows_as_the_badge_and_stays_across_refresh_without_a_new_read() {
	let ctx = egui::Context::default();
	let github = "https://github.com/team/thing.git";
	let (mut state, path) = with_origin("visibility-public", Some(github));
	let (key, checked) = remote_key(github);
	assert!(checked.is_ok(), "a GitHub origin is accepted");
	state.visibility.checking.insert(key.clone());
	let waiting = frame(&ctx, &mut state, Vec::new());
	assert!(
		has(&waiting, "Checking visibility…"),
		"{:?}",
		texts(&waiting)
	);
	state.merge_visibility(key.clone(), Visibility::Public);
	let shown = frame(&ctx, &mut state, Vec::new());
	assert!(has(&shown, "Visibility: Public"), "{:?}", texts(&shown));
	state.start_scan(&ctx);
	assert_eq!(
		held(&state),
		std::slice::from_ref(&key),
		"Refresh keeps the answer and asks gh nothing"
	);
	// Refresh re-reads the remotes; once origin is back, so is its badge, under its scp form too.
	state.remotes.insert(
		path,
		Ok(vec![RemoteEntry {
			name: "origin".into(),
			url: "git@github.com:team/thing.git".into(),
			push_url: None,
		}]),
	);
	state.merge_visibility(key, Visibility::Private);
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
