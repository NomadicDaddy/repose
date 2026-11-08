//! light-theme clause 4: switching theme while a diff is open keeps where the user was. The
//! switch is egui's own theme preference, the same one the Windows app theme drives through
//! winit, so the next frame draws the other token set with nothing in the app state reset.

use std::time::SystemTime;

use eframe::egui::epaint::{ClippedShape, Shape};
use eframe::egui::{self, pos2};
use repose_core::config::{Config, Theme};
use repose_core::git::parse::parse_unified;
use repose_core::model::{DiffSource, FileDiff};

use super::diff::colour_file;
use super::ui_harness::{frame, offset, pane_rects, repo, settle, wheel};
use super::{AppState, DiffContent, DiffState, OpenDiff};
use crate::diff_rows::build_rows;
use crate::screens::{diff, repo_list};
use crate::theme::{ADDED_BG, Colour, DIFF_ROW_HEIGHT};

/// A one-file Rust diff of `lines` added lines, alternating a comment and a function, so the
/// rows carry syntax colours; 200 is long enough to scroll inside the harness window.
fn rust_diff(lines: usize) -> Vec<FileDiff> {
	let mut text =
		format!("diff --git a/x.rs b/x.rs\n--- a/x.rs\n+++ b/x.rs\n@@ -1,0 +1,{lines} @@\n");
	for i in 0..lines {
		if i % 2 == 0 {
			text.push_str(&format!("+// note {i}\n"));
		} else {
			text.push_str(&format!("+fn f{i}() {{}}\n"));
		}
	}
	parse_unified(text.as_bytes())
}

/// Whether the frame drew the open diff's added lines on this fill.
fn added_lines_on(shapes: &[ClippedShape], fill: egui::Color32) -> bool {
	pane_rects(shapes, DIFF_ROW_HEIGHT)
		.iter()
		.any(|(_, f, _)| *f == fill)
}

/// Whether any text the frame drew has a run in this colour.
fn text_in(shapes: &[ClippedShape], colour: egui::Color32) -> bool {
	fn walk(shape: &Shape, colour: egui::Color32) -> bool {
		match shape {
			Shape::Vec(inner) => inner.iter().any(|s| walk(s, colour)),
			Shape::Text(t) => t
				.galley
				.job
				.sections
				.iter()
				.any(|s| s.format.color == colour),
			_ => false,
		}
	}
	shapes.iter().any(|c| walk(&c.shape, colour))
}

/// The first syntax colour of the open diff.
fn first_span(state: &AppState) -> Option<Colour> {
	let d = state.diff.as_ref()?;
	d.colours
		.iter()
		.flatten()
		.flatten()
		.flatten()
		.next()
		.map(|s| s.color)
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
	state.selected_file = Some("x.rs".into());
	let files = rust_diff(200);
	let rows = vec![build_rows(&files[0], false)];
	let colours = files.iter().map(colour_file).collect();
	let open = OpenDiff {
		repo: chosen.clone(),
		source: DiffSource::Working,
		paths: vec!["x.rs".into()],
		display: "x.rs".into(),
	};
	state.diff = Some(DiffState {
		open: open.clone(),
		content: DiffContent::Files(files),
		inline: false,
		generation: 1,
		rows,
		rows_inline: false,
		colours,
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
	let Some(span) = first_span(&state) else {
		panic!("the Rust diff carries syntax colours");
	};
	assert_ne!(span.dark, span.light, "the span differs by theme");
	assert!(text_in(&dark, span.dark), "dark draws the dark span colour");
	assert!(!text_in(&dark, span.light), "and not the light one");

	ctx.set_theme(egui::ThemePreference::Light);
	settle(&ctx, &mut state);
	let light = frame(&ctx, &mut state, Vec::new());
	assert!(
		added_lines_on(&light, ADDED_BG.light),
		"the switch drew light"
	);
	assert!(!added_lines_on(&light, ADDED_BG.dark), "and no dark line");
	assert!(
		text_in(&light, span.light),
		"the switch drew the light span colour"
	);
	assert!(!text_in(&light, span.dark), "and not the dark one");
	assert_eq!(
		first_span(&state),
		Some(span),
		"the colours were not worked out again"
	);

	assert_eq!(state.selected, Some(chosen), "the selected repository");
	assert_eq!(
		state.selected_file.as_deref(),
		Some("x.rs"),
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
