//! Whole frames drawn through the real screens on an egui context, with no window; the fixtures
//! and frame helpers live in ui_harness.rs. The scroll offsets are read back through the ids the
//! two panes publish each frame in test builds.

use std::time::{Duration, SystemTime};

use eframe::egui::{self, pos2};
use repose_core::config::Config;
use repose_core::model::{ChangeKind, DiffSource, FileChange, StatusSummary};

use super::diff::hash_bytes;
use super::ui_harness::{diff_of, frame, landed, offset, real_repo, repo, repo_at, settle, wheel};
use super::{AppState, DiffContent, DiffState, Msg, OpenDiff};
use crate::diff_rows::build_rows;
use crate::screens::{diff, repo_list};

/// A selection the user did not click (a command-line or start-folder selection landing with
/// discovery, a keyboard move) scrolls its row into view once; a later refresh does not scroll
/// again (Morticia L-2 on 4b39403).
#[test]
fn a_landed_selection_is_scrolled_into_view_once() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	let rows: Vec<_> = (0..40).map(|i| repo(&format!("r{i:02}"), t0)).collect();
	let far = rows[30].path.clone();
	state.requested_selection = Some(far.clone());
	assert!(
		state.merge_discovered(rows.clone()),
		"the requested row is found"
	);
	assert_eq!(state.selected, Some(far.clone()));
	assert!(state.reveal_selected, "a landed selection asks to be shown");
	frame(&ctx, &mut state, Vec::new());
	settle(&ctx, &mut state);
	let shown = offset(&ctx, repo_list::scroll_key());
	assert!(
		shown.y > 0.0,
		"row 30 of 40 is below the fold until scrolled to: {shown:?}"
	);
	assert!(!state.reveal_selected, "the ask is spent after one frame");
	// A refresh that re-lists the same rows keeps the selection and does not scroll again, even
	// after the user wheels away from it.
	frame(&ctx, &mut state, wheel(pos2(150.0, 250.0)));
	settle(&ctx, &mut state);
	let moved = offset(&ctx, repo_list::scroll_key());
	assert_ne!(moved, shown, "the wheel moved the list");
	state.merge_discovered(rows);
	assert!(
		!state.reveal_selected,
		"an unchanged selection does not ask again"
	);
	settle(&ctx, &mut state);
	assert_eq!(
		offset(&ctx, repo_list::scroll_key()),
		moved,
		"the list stayed where the wheel left it"
	);
	// A keyboard move asks again.
	state.move_selection(-25);
	assert!(state.reveal_selected);
	settle(&ctx, &mut state);
	assert!(!state.reveal_selected);
}

/// live-refresh clause 5: a status that arrives for the selected repository, through the same
/// message path a watcher refresh uses, leaves the selected repository, the selected file, the
/// open diff and both scroll positions where they were.
#[test]
fn refresh_preserves_selection() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let t0 = SystemTime::now();
	state.repos = (0..40).map(|i| repo(&format!("r{i:02}"), t0)).collect();
	// The selected row is a real repository with x.txt committed and unchanged, so the re-read a
	// status starts really runs git, gets empty bytes, finds their hash already known and sends
	// nothing back: no result can race the synthetic ones below (Roger 5Q-1 on f7a7f8c).
	let real = real_repo("r07");
	state.repos[7] = repo_at(real.clone(), t0);
	let chosen = state.repos[7].path.clone();
	state.selected = Some(chosen.clone());
	state.selected_file = Some("x.txt".into());
	let files = diff_of(200);
	assert_eq!(files.len(), 1, "the fixture parses to one file");
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
		raw_hash: Some(hash_bytes(b"")),
	});
	// The serial behind diff generations, as open_diff_in would have left it for generation 1.
	state.diff_serial = 1;

	// Lay out, then scroll each pane with the pointer over it. egui animates a wheel scroll over
	// several frames, so each reading waits for the offsets to settle.
	frame(&ctx, &mut state, Vec::new());
	frame(&ctx, &mut state, wheel(pos2(150.0, 250.0)));
	frame(&ctx, &mut state, wheel(pos2(800.0, 300.0)));
	settle(&ctx, &mut state);
	let list_before = offset(&ctx, repo_list::scroll_key());
	let diff_before = offset(&ctx, diff::scroll_key());
	assert!(list_before.y > 0.0, "the list scrolled: {list_before:?}");
	assert!(diff_before.y > 0.0, "the diff scrolled: {diff_before:?}");

	// A refresh of the selected repository: a newer status with a different file list.
	let mut fresh = repo_at(real.clone(), t0 + Duration::from_secs(1));
	fresh.status = Some(StatusSummary {
		unstaged: 2,
		..StatusSummary::default()
	});
	if let Some(c) = fresh.changes.as_mut() {
		c.unstaged.push(FileChange {
			path: "y.txt".into(),
			kind: ChangeKind::Modified,
			from: None,
		});
	}
	state
		.tx
		.send(Msg::Status(state.generation, Box::new(fresh)))
		.unwrap_or_else(|e| panic!("{e}"));
	state.pump(&ctx);
	// That status started a real re-read of the open diff; see the fixture above for why it
	// sends nothing. The settle frames give it time to run and prove the queue stays empty.
	settle(&ctx, &mut state);
	assert!(
		state.rx.try_recv().is_err(),
		"the real re-read of an unchanged diff sent nothing"
	);

	assert_eq!(
		state.selected,
		Some(chosen.clone()),
		"the selected repository"
	);
	assert_eq!(
		state.selected_file.as_deref(),
		Some("x.txt"),
		"the selected file"
	);
	let Some(d) = state.diff.as_ref() else {
		panic!("the diff is still open");
	};
	assert_eq!(d.open, open, "the same diff");
	assert!(
		matches!(d.content, DiffContent::Files(_)),
		"its content stands"
	);
	assert_eq!(
		d.rows.len(),
		1,
		"its rows stand while the re-read is in flight"
	);
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
	let unstaged = super::find(&state.repos, &chosen)
		.and_then(|r| r.status.as_ref())
		.map(|s| s.unstaged);
	assert_eq!(unstaged, Some(2), "the status was applied");

	// The re-read lands with different bytes (Morticia Low on caf22ba): a longer diff replaces
	// content, rows and colours, and the view stays where it was.
	let generation = d.generation;
	assert!(
		generation > 1,
		"the status started a re-read under a new generation"
	);
	state
		.tx
		.send(landed(generation, 240, 2))
		.unwrap_or_else(|e| panic!("{e}"));
	state.pump(&ctx);
	settle(&ctx, &mut state);
	let Some(d) = state.diff.as_ref() else {
		panic!("the diff is still open after the re-read landed");
	};
	assert!(
		matches!(&d.content, DiffContent::Files(f) if f[0].hunks[0].lines.len() == 240),
		"the longer diff is shown"
	);
	assert_eq!(d.raw_hash, Some(2));
	assert!(
		d.rows.first().is_some_and(|r| r.len() >= 240),
		"its rows were replaced by the longer layout"
	);
	assert_eq!(
		offset(&ctx, repo_list::scroll_key()),
		list_before,
		"the list scroll after the longer diff"
	);
	assert_eq!(
		offset(&ctx, diff::scroll_key()),
		diff_before,
		"the diff scroll after the longer diff"
	);

	// A shorter diff than the scrolled distance is the one expected exception: egui clamps the
	// offset to what is left to scroll, and nothing else moves.
	state
		.tx
		.send(landed(generation, 12, 3))
		.unwrap_or_else(|e| panic!("{e}"));
	state.pump(&ctx);
	settle(&ctx, &mut state);
	let short = offset(&ctx, diff::scroll_key());
	assert!(
		short.y < diff_before.y,
		"a twelve-line diff cannot scroll as far: {short:?} against {diff_before:?}"
	);
	assert_eq!(
		offset(&ctx, repo_list::scroll_key()),
		list_before,
		"the list scroll after the shorter diff"
	);
	assert_eq!(
		state.selected,
		Some(chosen),
		"the selection after both re-reads"
	);
}
