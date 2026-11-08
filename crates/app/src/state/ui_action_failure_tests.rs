//! audit-frontend-1793999566: an external action that fails says so. Reveal in Explorer on the
//! too-large diff, a change row's Open and Enter on the repository list each put the platform's
//! error in the notice under the repository header, as the row menu already did, and the next
//! frame paints it. The headless platform fails every action, so its "headless" is the text.

use std::path::Path;
use std::time::SystemTime;

use eframe::egui::{self, Key};
use repose_core::config::Config;
use repose_core::model::DiffSource;

use super::ui_harness::{Headless, click, frame, frame_on, key, repo_at, text_at, texts};
use super::{AppState, DiffContent, DiffState, OpenDiff};
use crate::platform::Platform;

/// A platform on which every action succeeds, to show that a success clears the notice.
struct Opens;

impl Platform for Opens {
	fn open_folder(&self, _dir: &Path) -> Result<(), String> {
		Ok(())
	}
	fn reveal_file(&self, _file: &Path) -> Result<(), String> {
		Ok(())
	}
	fn open_in_editor(&self, _c: &str, _t: &Path, _l: Option<usize>) -> Result<(), String> {
		Ok(())
	}
	fn open_terminal(&self, _c: &str, _dir: &Path) -> Result<(), String> {
		Ok(())
	}
}

fn selected(name: &str) -> AppState {
	let mut state = AppState::new(Config::default());
	let row = repo_at(
		std::env::temp_dir().join("repose-ui").join(name),
		SystemTime::now(),
	);
	state.selected = Some(row.path.clone());
	state.repos = vec![row];
	state
}

/// Clicks where the last frame painted `label`, on `platform`, then draws one more frame and
/// returns whether it painted the headless platform's error.
fn click_label(
	ctx: &egui::Context,
	state: &mut AppState,
	label: &str,
	platform: &dyn Platform,
) -> bool {
	let shapes = frame(ctx, state, Vec::new());
	let at = text_at(&shapes, label).unwrap_or_else(|| panic!("{label} not painted"));
	frame_on(ctx, state, click(at), platform);
	texts(&frame(ctx, state, Vec::new()))
		.iter()
		.any(|t| t == "headless")
}

#[test]
fn reveal_on_a_too_large_diff_names_its_failure() {
	let mut state = selected("failure-too-large");
	let repo = state.selected.clone().unwrap_or_default();
	state.diff = Some(DiffState {
		open: OpenDiff {
			repo,
			source: DiffSource::Working,
			paths: vec!["x.txt".into()],
			display: "x.txt".into(),
		},
		content: DiffContent::TooLarge(64 * 1_048_576),
		inline: false,
		generation: 1,
		rows: Vec::new(),
		rows_inline: false,
		colours: Vec::new(),
		raw_hash: None,
	});
	let ctx = egui::Context::default();
	assert!(
		click_label(&ctx, &mut state, "Reveal in Explorer", &Headless),
		"the failure is painted"
	);
	assert_eq!(state.notice.as_deref(), Some("headless"));
	// The control: the same click on a platform that succeeds clears the notice.
	click_label(&ctx, &mut state, "Reveal in Explorer", &Opens);
	assert_eq!(state.notice, None, "a success clears the notice");
}

#[test]
fn open_on_a_change_row_names_its_failure() {
	let mut state = selected("failure-change-open");
	let ctx = egui::Context::default();
	// The row's actions are drawn while the pointer is in the row.
	let shapes = frame(&ctx, &mut state, Vec::new());
	let row = text_at(&shapes, "x.txt").unwrap_or_else(|| panic!("change row not painted"));
	frame(&ctx, &mut state, vec![egui::Event::PointerMoved(row)]);
	assert_eq!(state.notice, None, "nothing failed before the click");
	assert!(
		click_label(&ctx, &mut state, "Open", &Headless),
		"the failure is painted"
	);
	assert_eq!(state.notice.as_deref(), Some("headless"));
}

#[test]
fn enter_on_the_repository_list_names_its_failure() {
	let mut state = selected("failure-list-enter");
	let ctx = egui::Context::default();
	frame(&ctx, &mut state, Vec::new());
	assert_eq!(state.notice, None, "nothing failed before Enter");
	frame(&ctx, &mut state, key(Key::Enter));
	assert_eq!(state.notice.as_deref(), Some("headless"));
	let painted = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(
		painted.iter().any(|t| t == "headless"),
		"the failure is painted"
	);
}
