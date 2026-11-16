//! Visible field labels remain in the accessibility tree after users type into the fields, and a
//! submodule's line in Files is named as one, with hover text that matches what repose does.

use std::time::SystemTime;

use eframe::egui::{self, Event};
use repose_core::config::Config;
use repose_core::model::TreeEntry;

use super::ui_frame::{run, screen_input};
use super::ui_harness::{repo, text_at, texts};
use super::{AppState, FileRev, FilesState, View};
use crate::file_tree::build_folder_tree;
use crate::screens::files::SUBMODULE_HINT;

/// submodule-tooltip-matches-count-only clauses 1 and 3: the disabled submodule line's accessible
/// name says it is a submodule, and resting the pointer on it shows the hint, which names the
/// parent's actions and not "open it as its own repository", a step the list cannot take.
#[test]
fn a_submodule_line_is_named_and_its_hint_matches_what_repose_does() {
	let ctx = egui::Context::default();
	ctx.enable_accesskit();
	let mut state = AppState::new(Config::default());
	let row = repo("with-submodule", SystemTime::now());
	let path = row.path.clone();
	state.selected = Some(path.clone());
	state.merge_discovered(vec![row]);
	state.view = View::Files;
	let entries = [TreeEntry {
		path: "vendor".into(),
		size: 0,
		submodule: true,
	}];
	let files = FilesState {
		rev: FileRev::Head,
		loaded: Some(FileRev::Head),
		tree: build_folder_tree(&entries),
		..FilesState::default()
	};
	state.files.insert(path, files);
	let frame_at = |state: &mut AppState, events: Vec<Event>, time: f64| {
		let input = egui::RawInput {
			time: Some(time),
			..screen_input(events)
		};
		run(&ctx, state, input)
	};
	// The Files tree is laid out on the first frame and in the tree from the second; a Label
	// node carries its text as its value (as ui_a11y_tests reads it).
	frame_at(&mut state, Vec::new(), 0.0);
	let out = frame_at(&mut state, Vec::new(), 0.05);
	let named = out.platform_output.accesskit_update.is_some_and(|u| {
		u.nodes
			.iter()
			.any(|(_, n)| n.label().or(n.value()) == Some("vendor, submodule"))
	});
	assert!(named, "the line is named as a submodule");
	let at = text_at(&out.shapes, "vendor")
		.unwrap_or_else(|| panic!("no submodule line: {:?}", texts(&out.shapes)));
	let mut shown = false;
	for i in 1..=20 {
		let events = if i == 1 {
			vec![Event::PointerMoved(at)]
		} else {
			Vec::new()
		};
		let out = frame_at(&mut state, events, f64::from(i) * 0.1);
		shown |= texts(&out.shapes).iter().any(|t| t == SUBMODULE_HINT);
	}
	assert!(shown, "resting on the line shows the hint");
	assert!(!SUBMODULE_HINT.contains("open it as its own repository"));
	for action in [
		"Open folder in Explorer",
		"Open in VS Code",
		"Open a terminal here",
	] {
		assert!(SUBMODULE_HINT.contains(action), "the hint names {action}");
	}
}

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
