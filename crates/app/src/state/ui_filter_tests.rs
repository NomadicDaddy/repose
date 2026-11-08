//! The Find box through the real left pane: Escape with the box focused clears the filter
//! (repository-list clause 6). egui's text box gives focus up on Escape before the screen reads
//! the key, so the check has to accept a box that lost focus on that frame.

use std::time::SystemTime;

use eframe::egui::{self, Event, Key, Modifiers};
use repose_core::config::Config;

use super::AppState;
use super::ui_harness::{frame, repo, texts};

/// Design-polish clause 4: with a filter typed, the status line says how many repositories match,
/// and the detail pane says when the repository it shows is hidden from the list by the filter.
#[test]
fn a_typed_filter_counts_matches_and_names_a_hidden_selection() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	let deeper = repo("deeper", t0);
	state.requested_selection = Some(deeper.path.clone());
	state.merge_discovered(vec![repo("summon", t0), deeper]);
	let hidden = "Hidden from the list by the filter";
	// Control: no filter, so the plain count and no note.
	let all = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(
		all.iter().any(|t| t.starts_with("2 repositories")),
		"{all:?}"
	);
	assert!(!all.iter().any(|t| t.contains(hidden)), "{all:?}");
	// A filter that hides the selected repository.
	state.filter = "su".to_string();
	state.filter_changed();
	let all = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(all.iter().any(|t| t.starts_with("1 of 2 match")), "{all:?}");
	assert!(
		all.iter()
			.any(|t| t.contains(hidden) && t.contains("\"su\"")),
		"{all:?}"
	);
	// Control: a filter that keeps it shown says the count and no note.
	state.filter = "de".to_string();
	state.filter_changed();
	let all = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(all.iter().any(|t| t.starts_with("1 of 2 match")), "{all:?}");
	assert!(!all.iter().any(|t| t.contains(hidden)), "{all:?}");
}

fn key(key: Key, pressed: bool) -> Event {
	Event::Key {
		key,
		physical_key: None,
		pressed,
		repeat: false,
		modifiers: Modifiers::NONE,
	}
}

#[test]
fn escape_in_the_focused_find_box_clears_the_filter() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	state.merge_discovered(vec![repo("summon", t0), repo("deeper", t0)]);
	state.filter = "su".to_string();
	state.filter_changed();
	state.focus_filter = true;
	frame(&ctx, &mut state, Vec::new());
	frame(&ctx, &mut state, Vec::new());
	assert_eq!(state.filter, "su", "the filter stands until Escape");
	frame(
		&ctx,
		&mut state,
		vec![key(Key::Escape, true), key(Key::Escape, false)],
	);
	assert!(
		state.filter.is_empty(),
		"Escape in the focused box clears it, got {:?}",
		state.filter
	);
}

/// The empty Find box's hint uses the ellipsis character, as repository-list clause 12 says,
/// not three ASCII periods (audit-web-design-guidelines, filter placeholder).
#[test]
fn the_find_box_hint_uses_an_ellipsis() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	state.merge_discovered(vec![repo("summon", SystemTime::now())]);
	let shown = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(
		shown.iter().any(|t| t == "name or path\u{2026}"),
		"{shown:?}"
	);
	assert!(!shown.iter().any(|t| t.contains("...")), "{shown:?}");
}
