//! The open diff: which file from which source, read on a background thread after the size check,
//! and re-read when a write in that repository lands.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::diff::{MAX_DIFF_BYTES, blob_size, read_diff};
use repose_core::model::{DiffSource, FileDiff};

use super::{AppState, Msg};
use crate::diff_rows::Row;

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
}

impl AppState {
	/// Opens a diff in the right pane, keeping the Inline choice across files.
	pub fn open_diff(&mut self, open: OpenDiff, ctx: &egui::Context) {
		let (inline, generation) = match &self.diff {
			Some(d) if d.open == open => return,
			Some(d) => (d.inline, d.generation + 1),
			None => (false, 1),
		};
		self.diff = Some(DiffState {
			open: open.clone(),
			content: DiffContent::Loading,
			inline,
			generation,
			rows: Vec::new(),
			rows_inline: inline,
		});
		spawn_read(open, generation, self.tx.clone(), ctx.clone());
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
		d.generation += 1;
		spawn_read(d.open.clone(), d.generation, self.tx.clone(), ctx.clone());
	}

	pub(super) fn merge_diff(&mut self, msg: DiffMsg) {
		if let Some(d) = self.diff.as_mut()
			&& d.generation == msg.generation
		{
			d.content = msg.content;
			d.rows.clear();
		}
	}
}

/// The size of the side that exists: the working file, the index blob or the commit's blob.
fn side_size(open: &OpenDiff) -> Result<u64, String> {
	let path = &open.display;
	match &open.source {
		DiffSource::Working => {
			let full = open.repo.join(path.replace('/', "\\"));
			match std::fs::metadata(&full) {
				Ok(m) => Ok(m.len()),
				// A deleted file has no working side; the index blob is what the diff removes.
				Err(_) => blob_size(&open.repo, ":0", path).map_err(|e| e.to_string()),
			}
		}
		DiffSource::Staged => blob_size(&open.repo, ":0", path)
			.or_else(|_| blob_size(&open.repo, "HEAD", path))
			.map_err(|e| e.to_string()),
		DiffSource::Commit(hash) => blob_size(&open.repo, hash, path)
			.or_else(|_| blob_size(&open.repo, &format!("{hash}^"), path))
			.map_err(|e| e.to_string()),
	}
}

fn spawn_read(open: OpenDiff, generation: u64, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let content = match side_size(&open) {
			Ok(size) if size > MAX_DIFF_BYTES => DiffContent::TooLarge(size),
			// An unreadable size (an untracked file that vanished, a path git cannot name) falls
			// through to the diff itself, whose error says more.
			_ => match read_diff(&open.repo, &open.source, &open.paths) {
				Ok(files) => DiffContent::Files(files),
				Err(e) => DiffContent::Error(e.to_string()),
			},
		};
		let _ = tx.send(Msg::Diff(DiffMsg {
			generation,
			content,
		}));
		ctx.request_repaint();
	});
}
