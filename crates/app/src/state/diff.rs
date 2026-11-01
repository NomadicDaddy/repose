//! The open diff: which file from which source, read on a background thread after the size check,
//! and re-read when a write in that repository lands.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::diff::{MAX_DIFF_BYTES, blob_size, read_diff};
use repose_core::model::{DiffSource, FileDiff};

use super::{AppState, Msg};
use crate::diff_rows::{Row, build_rows};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenDiff {
	pub repo: PathBuf,
	pub source: DiffSource,
	/// Both paths of a rename, the one path otherwise; what git is asked for.
	pub paths: Vec<String>,
	/// The path shown in the breadcrumb (the new one).
	pub display: String,
	/// An untracked file has no diff; the pane says so instead of "No differences".
	pub untracked: bool,
}

pub enum DiffContent {
	Loading,
	TooLarge(u64),
	Files(Vec<FileDiff>),
	Error(String),
}

pub struct DiffState {
	pub open: OpenDiff,
	pub content: DiffContent,
	pub inline: bool,
	/// Bumped on every (re)load; a result from an older load is dropped.
	pub generation: u64,
	/// Drawable rows per file, built once per content and mode.
	pub rows: Vec<Vec<Row>>,
	pub rows_inline: bool,
}

pub struct DiffMsg {
	pub generation: u64,
	pub content: DiffContent,
	/// Rows built on the reader thread for the mode that was current when the read started.
	pub rows: Vec<Vec<Row>>,
	pub rows_inline: bool,
}

impl AppState {
	/// Opens a diff in the right pane, keeping the Inline choice across files.
	pub fn open_diff(&mut self, open: OpenDiff, ctx: &egui::Context) {
		let inline = match &self.diff {
			Some(d) if d.open == open => return,
			Some(d) => d.inline,
			None => false,
		};
		self.diff_serial += 1;
		let generation = self.diff_serial;
		self.diff = Some(DiffState {
			open: open.clone(),
			content: DiffContent::Loading,
			inline,
			generation,
			rows: Vec::new(),
			rows_inline: inline,
		});
		spawn_read(open, generation, inline, self.tx.clone(), ctx.clone());
	}

	pub fn close_diff(&mut self) {
		self.diff = None;
	}

	/// Re-reads the open diff after a write in its repository (a stage moves lines between the
	/// working and staged diffs) or a status change there.
	pub(super) fn reload_diff_for(&mut self, repo: &Path, ctx: &egui::Context) {
		let Some(d) = self.diff.as_mut() else {
			return;
		};
		if d.open.repo != repo {
			return;
		}
		self.diff_serial += 1;
		d.generation = self.diff_serial;
		spawn_read(
			d.open.clone(),
			d.generation,
			d.inline,
			self.tx.clone(),
			ctx.clone(),
		);
	}

	pub(super) fn merge_diff(&mut self, msg: DiffMsg) {
		if let Some(d) = self.diff.as_mut()
			&& d.generation == msg.generation
		{
			d.content = msg.content;
			d.rows = msg.rows;
			d.rows_inline = msg.rows_inline;
		}
	}
}

/// The largest side of the diff: the new side and the old side are both measured, since either
/// can be the big one. A side that does not exist (an added or deleted file) is skipped, but when
/// neither side can be measured nothing is sent to git: the size check fails closed.
fn largest_side(open: &OpenDiff) -> Result<u64, String> {
	let new_path = open.display.as_str();
	let old_path = open
		.paths
		.iter()
		.find(|p| p.as_str() != new_path)
		.map_or(new_path, String::as_str);
	let sides: Vec<Result<u64, String>> = match &open.source {
		DiffSource::Working => vec![
			std::fs::metadata(open.repo.join(new_path.replace('/', "\\")))
				.map(|m| m.len())
				.map_err(|e| e.to_string()),
			blob_size(&open.repo, ":0", old_path).map_err(|e| e.to_string()),
		],
		DiffSource::Staged => vec![
			blob_size(&open.repo, ":0", new_path).map_err(|e| e.to_string()),
			blob_size(&open.repo, "HEAD", old_path).map_err(|e| e.to_string()),
		],
		DiffSource::Commit(hash) => vec![
			blob_size(&open.repo, hash, new_path).map_err(|e| e.to_string()),
			blob_size(&open.repo, &format!("{hash}^"), old_path).map_err(|e| e.to_string()),
		],
	};
	let known: Vec<u64> = sides
		.iter()
		.filter_map(|s| s.as_ref().ok().copied())
		.collect();
	match known.iter().max() {
		Some(max) => Ok(*max),
		None => Err(format!(
			"Size unknown on both sides, so the diff was not run: {}",
			sides
				.iter()
				.filter_map(|s| s.as_ref().err().cloned())
				.collect::<Vec<_>>()
				.join(" / ")
		)),
	}
}

fn spawn_read(open: OpenDiff, generation: u64, inline: bool, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let content = match largest_side(&open) {
			Ok(size) if size > MAX_DIFF_BYTES => DiffContent::TooLarge(size),
			Ok(_) => match read_diff(&open.repo, &open.source, &open.paths) {
				Ok(files) => DiffContent::Files(files),
				Err(e) => DiffContent::Error(e.to_string()),
			},
			Err(e) => DiffContent::Error(e),
		};
		// Rows, with their word marks, are built here and not on the UI thread.
		let rows = match &content {
			DiffContent::Files(files) => files.iter().map(|f| build_rows(f, inline)).collect(),
			_ => Vec::new(),
		};
		let _ = tx.send(Msg::Diff(DiffMsg {
			generation,
			content,
			rows,
			rows_inline: inline,
		}));
		ctx.request_repaint();
	});
}
