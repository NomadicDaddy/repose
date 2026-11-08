//! audit-frontend-1793711157 clause 3: the branch row's Copy branch name menu opens from the
//! keyboard (the Menu key on the focused row), not only from a secondary click, and its one
//! action copies the name.

use std::time::SystemTime;

use eframe::egui::{self, Event, Key, OutputCommand};
use repose_core::config::Config;
use repose_core::model::{Branch, Branches, HeadState};

use super::ui_frame::{run, screen_input};
use super::ui_harness::{click, frame, key, repo, tab, text_at, texts};
use super::{AppState, BranchesState, View};

const LABEL: &str = "Copy branch name";

fn branch(name: &str, current: bool) -> Branch {
	Branch {
		name: name.into(),
		short_hash: "0123abc".into(),
		upstream: None,
		ahead: None,
		behind: None,
		current,
	}
}

/// One frame that returns the commands it asked the platform for (the clipboard among them).
fn commands(ctx: &egui::Context, state: &mut AppState, events: Vec<Event>) -> Vec<OutputCommand> {
	run(ctx, state, screen_input(events))
		.platform_output
		.commands
}

#[test]
fn the_menu_key_opens_copy_branch_name_on_the_focused_branch() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let row = repo("branches", SystemTime::now());
	let path = row.path.clone();
	state.merge_discovered(vec![row]);
	state.view = View::Branches;
	state.branches.insert(
		path,
		BranchesState {
			branches: Some(Branches {
				head: HeadState::OnBranch("main".into()),
				list: vec![branch("main", true), branch("topic", false)],
			}),
			..BranchesState::default()
		},
	);
	frame(&ctx, &mut state, Vec::new());
	// Tab until a branch row holds focus; the Branches tab marks that with rows_focus each frame.
	let mut reached = false;
	for _ in 0..40 {
		frame(&ctx, &mut state, tab());
		if state.rows_focus {
			reached = true;
			break;
		}
	}
	assert!(reached, "Tab did not reach a branch row in 40 presses");
	let before = frame(&ctx, &mut state, Vec::new());
	assert!(
		!texts(&before).iter().any(|t| t == LABEL),
		"no menu before the Menu key"
	);
	frame(&ctx, &mut state, key(Key::Menu));
	let open = frame(&ctx, &mut state, Vec::new());
	let at = text_at(&open, LABEL)
		.unwrap_or_else(|| panic!("the Menu key did not open {LABEL:?}: {:?}", texts(&open)));
	let copied = commands(&ctx, &mut state, click(at));
	assert!(
		copied
			.iter()
			.any(|c| matches!(c, OutputCommand::CopyText(t) if t == "main" || t == "topic")),
		"the menu item copied a branch name: {copied:?}"
	);
}
