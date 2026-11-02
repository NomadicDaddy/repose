//! The Find box through the real left pane: Escape with the box focused clears the filter
//! (repository-list clause 6). egui's text box gives focus up on Escape before the screen reads
//! the key, so the check has to accept a box that lost focus on that frame.

use std::time::SystemTime;

use eframe::egui::{self, Event, Key, Modifiers};
use repose_core::config::Config;

use super::AppState;
use super::ui_harness::{frame, repo};

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
