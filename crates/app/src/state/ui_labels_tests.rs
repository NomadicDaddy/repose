//! Visible field labels remain in the accessibility tree after users type into the fields.

use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;

use super::AppState;
use super::ui_frame::{run, screen_input};
use super::ui_harness::repo;

#[test]
fn find_and_commit_fields_have_names_with_nonempty_buffers() {
	let ctx = egui::Context::default();
	ctx.enable_accesskit();
	let mut state = AppState::new(Config::default());
	let selected = repo("labels", SystemTime::now());
	state.filter = "labels".into();
	state.selected = Some(selected.path.clone());
	state
		.commit_messages
		.insert(selected.path.clone(), "draft".into());
	state.repos.push(selected);
	let output = run(&ctx, &mut state, screen_input(Vec::new()));
	let Some(update) = output.platform_output.accesskit_update else {
		panic!("accessibility tree was not emitted");
	};
	for name in ["Find a repository", "Commit message"] {
		assert!(
			update
				.nodes
				.iter()
				.any(|(_, node)| { node.label() == Some(name) && !node.labelled_by().is_empty() }),
			"{name} has a direct name and visible-label relation"
		);
	}
}
