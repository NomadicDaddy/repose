//! audit-frontend-1793999566: Escape answers the dialog that stands, and only the dialog. With a
//! diff or a file open behind the discard confirmation or the close question, one Escape closes
//! the dialog and leaves the pane as it was. With no dialog, Escape still closes the pane.

use std::path::PathBuf;
use std::time::SystemTime;

use eframe::egui::{self, Key};
use repose_core::config::Config;
use repose_core::model::DiffSource;

use super::diff::hash_bytes;
use super::ui_harness::{diff_of, frame, key, real_repo, repo_at};
use super::{
	AppState, DiffContent, DiffState, FileContent, FileRev, FileViewState, OpenDiff, OpenFile,
	PendingDiscard,
};
use crate::diff_rows::build_rows;

#[derive(Clone, Copy)]
enum Pane {
	Diff,
	File,
}

#[derive(Clone, Copy)]
enum Dialog {
	None,
	Discard,
	Close,
}

fn with_pane(name: &str, pane: Pane) -> (AppState, PathBuf) {
	let path = real_repo(name);
	let mut state = AppState::new(Config::default());
	state.repos = vec![repo_at(path.clone(), SystemTime::now())];
	state.selected = Some(path.clone());
	match pane {
		Pane::Diff => {
			let files = diff_of(3);
			let rows = vec![build_rows(&files[0], false)];
			state.diff = Some(DiffState {
				open: OpenDiff {
					repo: path.clone(),
					source: DiffSource::Working,
					paths: vec!["x.txt".into()],
					display: "x.txt".into(),
				},
				content: DiffContent::Files(files),
				inline: false,
				generation: 1,
				rows,
				rows_inline: false,
				colours: Vec::new(),
				raw_hash: Some(hash_bytes(b"")),
			});
			state.diff_serial = 1;
		}
		Pane::File => {
			state.file_view = Some(FileViewState {
				open: OpenFile {
					repo: path.clone(),
					rev: FileRev::Head,
					path: "x.txt".into(),
					size: Some(6),
				},
				content: FileContent::Text {
					lines: vec!["first".into()],
					colours: Vec::new(),
					columns: 5,
				},
				generation: 0,
				current_line: Some(1),
				notice: None,
			});
		}
	}
	(state, path)
}

fn open_dialog(state: &mut AppState, path: PathBuf, dialog: Dialog) {
	match dialog {
		Dialog::None => {}
		Dialog::Discard => {
			state.pending_discard = Some(PendingDiscard {
				repo: path,
				tracked: vec!["x.txt".into()],
				untracked: Vec::new(),
				return_focus: None,
				focus_cancel: true,
			});
		}
		Dialog::Close => state.close_prompt = Some(vec!["repo".into()]),
	}
}

/// Opens `pane`, stands `dialog` over it for two frames, presses Escape once, and returns
/// whether the dialog and the pane are still there afterwards.
fn escape_over(name: &str, pane: Pane, dialog: Dialog) -> (bool, bool) {
	let (mut state, path) = with_pane(name, pane);
	open_dialog(&mut state, path, dialog);
	let ctx = egui::Context::default();
	frame(&ctx, &mut state, Vec::new());
	frame(&ctx, &mut state, Vec::new());
	frame(&ctx, &mut state, key(Key::Escape));
	let pane_open = match pane {
		Pane::Diff => state.diff.is_some(),
		Pane::File => state.file_view.is_some(),
	};
	(state.modal_open(), pane_open)
}

#[test]
fn escape_with_no_dialog_closes_the_diff_and_the_file() {
	assert_eq!(
		escape_over("escape-diff-alone", Pane::Diff, Dialog::None),
		(false, false),
		"Escape closes the diff"
	);
	assert_eq!(
		escape_over("escape-file-alone", Pane::File, Dialog::None),
		(false, false),
		"Escape closes the file"
	);
}

#[test]
fn escape_on_the_discard_dialog_keeps_the_pane_behind_it() {
	assert_eq!(
		escape_over("escape-discard-diff", Pane::Diff, Dialog::Discard),
		(false, true),
		"the dialog closes and the diff stays"
	);
	assert_eq!(
		escape_over("escape-discard-file", Pane::File, Dialog::Discard),
		(false, true),
		"the dialog closes and the file stays"
	);
}

#[test]
fn escape_on_the_close_question_keeps_the_pane_behind_it() {
	assert_eq!(
		escape_over("escape-close-diff", Pane::Diff, Dialog::Close),
		(false, true),
		"Keep editing and the diff stays"
	);
	assert_eq!(
		escape_over("escape-close-file", Pane::File, Dialog::Close),
		(false, true),
		"Keep editing and the file stays"
	);
}
