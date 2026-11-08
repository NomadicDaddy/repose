//! light-theme clause 4: switching theme while a diff is open keeps where the user was. The
//! switch is egui's own theme preference, the same one the Windows app theme drives through
//! winit, so the next frame draws the other token set with nothing in the app state reset.

use std::time::SystemTime;

use eframe::egui::{self, pos2};
use repose_core::config::{Config, Theme};
use repose_core::model::DiffSource;

use super::ui_harness::{diff_of, frame, offset, pane_rects, repo, settle, wheel};
use super::{AppState, DiffContent, DiffState, OpenDiff};
use crate::diff_rows::build_rows;
use crate::screens::{diff, repo_list};
use crate::theme::{ADDED_BG, DIFF_ROW_HEIGHT};

/// Whether the frame drew the open diff's added lines on this fill.
fn added_lines_on(shapes: &[egui::epaint::ClippedShape], fill: egui::Color32) -> bool {
	pane_rects(shapes, DIFF_ROW_HEIGHT)
		.iter()
		.any(|(_, f, _)| *f == fill)
}

#[test]
fn switching_theme_with_a_diff_open_keeps_the_scroll_and_the_selection() {
	let ctx = egui::Context::default();
	crate::theme::apply(&ctx, Theme::Dark);
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	state.repos = (0..40).map(|i| repo(&format!("r{i:02}"), t0)).collect();
	let chosen = state.repos[7].path.clone();
	state.selected = Some(chosen.clone());
	state.selected_file = Some("x.txt".into());
	let files = diff_of(200);
	let rows = vec![build_rows(&files[0], false)];
	let open = OpenDiff {
		repo: chosen.clone(),
		source: DiffSource::Working,
		paths: vec!["x.txt".into()],
		display: "x.txt".into(),
	};
	state.diff = Some(DiffState {
		open: open.clone(),
		content: DiffContent::Files(files),
		inline: false,
		generation: 1,
		rows,
		rows_inline: false,
		colours: Vec::new(),
		raw_hash: None,
	});
	state.diff_serial = 1;

	frame(&ctx, &mut state, Vec::new());
	frame(&ctx, &mut state, wheel(pos2(150.0, 250.0)));
	frame(&ctx, &mut state, wheel(pos2(800.0, 300.0)));
	settle(&ctx, &mut state);
	let list_before = offset(&ctx, repo_list::scroll_key());
	let diff_before = offset(&ctx, diff::scroll_key());
	assert!(list_before.y > 0.0, "the list scrolled: {list_before:?}");
	assert!(diff_before.y > 0.0, "the diff scrolled: {diff_before:?}");
	let dark = frame(&ctx, &mut state, Vec::new());
	assert!(added_lines_on(&dark, ADDED_BG.dark), "dark draws dark");
	assert!(!added_lines_on(&dark, ADDED_BG.light), "and no light line");

	ctx.set_theme(egui::ThemePreference::Light);
	settle(&ctx, &mut state);
	let light = frame(&ctx, &mut state, Vec::new());
	assert!(
		added_lines_on(&light, ADDED_BG.light),
		"the switch drew light"
	);
	assert!(!added_lines_on(&light, ADDED_BG.dark), "and no dark line");

	assert_eq!(state.selected, Some(chosen), "the selected repository");
	assert_eq!(
		state.selected_file.as_deref(),
		Some("x.txt"),
		"the selected file"
	);
	let Some(d) = state.diff.as_ref() else {
		panic!("the diff is still open");
	};
	assert_eq!(d.open, open, "the same diff");
	assert_eq!(d.generation, 1, "not re-read");
	assert_eq!(
		offset(&ctx, repo_list::scroll_key()),
		list_before,
		"the list scroll"
	);
	assert_eq!(
		offset(&ctx, diff::scroll_key()),
		diff_before,
		"the diff scroll"
	);
	crate::theme::set_light(false);
}
