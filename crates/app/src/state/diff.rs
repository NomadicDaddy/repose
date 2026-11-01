//! The open diff: which file from which source, read on a background thread after the size check,
//! and re-read when a write in that repository lands.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::diff::{MAX_DIFF_BYTES, blob_size, read_diff};
use repose_core::model::{DiffSource, FileDiff, LineKind};

use super::{AppState, Msg};
use crate::diff_rows::{Row, build_rows};
use crate::highlight::{Span, colour_lines};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenDiff {
	pub repo: PathBuf,
	pub source: DiffSource,
	/// Both paths of a rename, the one path otherwise; what git is asked for.
	pub paths: Vec<String>,
	/// The path shown in the breadcrumb (the new one).
	pub display: String,
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
	/// Syntax colour spans per file, hunk and line, built with the rows.
	pub colours: Vec<FileColours>,
}

/// Colour spans indexed like the file's hunks and their lines.
pub type FileColours = Vec<Vec<Vec<Span>>>;

pub struct DiffMsg {
	pub generation: u64,
	pub content: DiffContent,
	/// Rows built on the reader thread for the mode that was current when the read started.
	pub rows: Vec<Vec<Row>>,
	pub rows_inline: bool,
	pub colours: Vec<FileColours>,
}

/// Colours every line of a file hunk by hunk, with one parser per side: the old side is context
/// plus removed lines and the new side context plus added, so a removed line that opens a block
/// comment cannot colour the added lines after it (Morticia L-1 on e8e10ad). Context lines take
/// the new side's colours.
fn colour_file(file: &FileDiff) -> FileColours {
	let path = file.path().to_string();
	file.hunks
		.iter()
		.map(|h| {
			let side = |skip: LineKind| -> (Vec<usize>, Vec<&str>) {
				h.lines
					.iter()
					.enumerate()
					.filter(|(_, l)| l.kind != skip)
					.map(|(i, l)| (i, l.text.as_str()))
					.unzip()
			};
			let (old_idx, old_lines) = side(LineKind::Added);
			let (new_idx, new_lines) = side(LineKind::Removed);
			let old = colour_lines(&path, &old_lines);
			let new = colour_lines(&path, &new_lines);
			let mut out: Vec<Vec<Span>> = vec![Vec::new(); h.lines.len()];
			for (i, spans) in old_idx.into_iter().zip(old) {
				out[i] = spans;
			}
			for (i, spans) in new_idx.into_iter().zip(new) {
				out[i] = spans;
			}
			out
		})
		.collect()
}

impl AppState {
	/// Opens a diff in the right pane, keeping the Inline choice across files.
	pub fn open_diff(&mut self, open: OpenDiff, ctx: &egui::Context) {
		let inline = self.diff.as_ref().is_some_and(|d| d.inline);
		self.open_diff_in(open, inline, ctx);
	}

	/// Opens a diff in the given mode, so the first read already lays out the mode asked for.
	pub fn open_diff_in(&mut self, open: OpenDiff, inline: bool, ctx: &egui::Context) {
		if self
			.diff
			.as_ref()
			.is_some_and(|d| d.open == open && d.inline == inline)
		{
			return;
		}
		self.diff_serial += 1;
		let generation = self.diff_serial;
		self.diff = Some(DiffState {
			open: open.clone(),
			content: DiffContent::Loading,
			inline,
			generation,
			rows: Vec::new(),
			rows_inline: inline,
			colours: Vec::new(),
		});
		spawn_read(open, generation, inline, self.tx.clone(), ctx.clone());
	}

	pub fn close_diff(&mut self) {
		self.diff = None;
	}

	/// Switches between side by side and inline by restarting the read in the new mode under a
	/// new serial. Re-reading git costs one diff; it means a read still in flight (first load or
	/// a reload) is simply superseded, so the rows that land always match the mode shown, and the
	/// UI thread never lays out a diff (Morticia on 54be14f, Roger on 15ce293).
	pub fn toggle_inline(&mut self, ctx: &egui::Context) {
		let Some(d) = self.diff.as_mut() else {
			return;
		};
		d.inline = !d.inline;
		let repo = d.open.repo.clone();
		self.reload_diff_for(&repo, ctx);
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
			d.colours = msg.colours;
		}
	}
}

/// One side of a diff as the size check sees it.
enum Side {
	Size(u64),
	/// The side does not exist: an added or deleted file. Git or the file system said so.
	Absent,
	/// Could not be measured for any other reason (timeout, lost output, unreadable repository).
	Unknown(String),
}

fn fs_side(path: &Path) -> Side {
	match std::fs::metadata(path) {
		Ok(m) => Side::Size(m.len()),
		Err(e) if e.kind() == std::io::ErrorKind::NotFound => Side::Absent,
		Err(e) => Side::Unknown(e.to_string()),
	}
}

/// Absence comes from core's exit-code check (unborn HEAD, a root commit's parent, a conflicted
/// path's stage 0 and a missing path all read as None); any error is Unknown and stops the read.
fn blob_side(repo: &Path, rev: &str, path: &str) -> Side {
	match blob_size(repo, rev, path) {
		Ok(Some(n)) => Side::Size(n),
		Ok(None) => Side::Absent,
		Err(e) => Side::Unknown(e.to_string()),
	}
}

/// The largest side of the diff: the new side and the old side are both measured, since either
/// can be the big one. A side that does not exist (an added or deleted file) is skipped; a side
/// that cannot be measured stops the read, because an unknown side may be the big one. Nothing
/// is sent to git unless every existing side has a known size.
fn largest_side(open: &OpenDiff) -> Result<u64, String> {
	let new_path = open.display.as_str();
	let old_path = open
		.paths
		.iter()
		.find(|p| p.as_str() != new_path)
		.map_or(new_path, String::as_str);
	let sides = match &open.source {
		DiffSource::Working => [
			fs_side(&open.repo.join(new_path.replace('/', "\\"))),
			blob_side(&open.repo, ":0", old_path),
		],
		DiffSource::Staged => [
			blob_side(&open.repo, ":0", new_path),
			blob_side(&open.repo, "HEAD", old_path),
		],
		DiffSource::Commit(hash) => [
			blob_side(&open.repo, hash, new_path),
			blob_side(&open.repo, &format!("{hash}^"), old_path),
		],
	};
	let mut max: Option<u64> = None;
	for side in &sides {
		match side {
			Side::Size(n) => max = Some(max.map_or(*n, |m| m.max(*n))),
			Side::Absent => {}
			Side::Unknown(why) => {
				return Err(format!(
					"A side's size could not be read, so the diff was not run: {why}"
				));
			}
		}
	}
	match max {
		Some(n) => Ok(n),
		None => Err("Neither side exists any more; nothing to diff.".to_string()),
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
		// Rows, with their word marks and syntax colours, are built here and not on the UI thread.
		let (rows, colours) = match &content {
			DiffContent::Files(files) => (
				files.iter().map(|f| build_rows(f, inline)).collect(),
				files.iter().map(colour_file).collect(),
			),
			_ => (Vec::new(), Vec::new()),
		};
		let _ = tx.send(Msg::Diff(DiffMsg {
			generation,
			content,
			rows,
			rows_inline: inline,
			colours,
		}));
		ctx.request_repaint();
	});
}
