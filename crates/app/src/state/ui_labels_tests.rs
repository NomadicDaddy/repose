//! Visible field labels remain in the accessibility tree after users type into the fields.

use std::time::SystemTime;

use eframe::egui::{self, Pos2, Rect};
use repose_core::config::Config;

use super::AppState;
use super::ui_harness::{Headless, SCREEN, repo};
use crate::screens::{detail, repo_list};

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
	let input = egui::RawInput {
		screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
		..Default::default()
	};
	let mut output = ctx.run_ui(input, |root| {
		repo_list::show(root, &mut state, &Headless);
		detail::show(root, &mut state, &Headless);
	});
	output.textures_delta.clear();
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
