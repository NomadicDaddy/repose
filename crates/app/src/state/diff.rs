//! The open diff: which file from which source, read on a background thread after the size check,
//! and re-read when a write in that repository lands.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::diff::{MAX_DIFF_BYTES, read_diff_bytes};
use repose_core::git::parse::parse_unified;
use repose_core::model::{DiffSource, FileDiff, LineKind};

use super::diff_size::largest_side;
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
	/// A hash of git's bytes behind `content`, so a re-read that produced the same diff sends
	/// nothing back and the rows and colours stand.
	pub raw_hash: Option<u64>,
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
	pub raw_hash: Option<u64>,
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
		self.close_file();
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
			raw_hash: None,
		});
		spawn_read(open, generation, inline, None, self.tx.clone(), ctx.clone());
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
			None,
			self.tx.clone(),
			ctx.clone(),
		);
	}

	/// After a status read in the diff's repository: a working-tree or staged diff is re-read
	/// every time, because a second edit to an already-modified file changes the diff and
	/// nothing in the status (Morticia M-1 on 9007389); the reader sends nothing back when git's
	/// bytes are the same, so an unchanged pass costs one git read and no re-layout (her L-2 on
	/// 39951dc). A commit's diff only follows a status that changed something.
	pub(super) fn reload_diff_after_status(
		&mut self,
		repo: &Path,
		changed: bool,
		ctx: &egui::Context,
	) {
		let Some(d) = self.diff.as_mut() else {
			return;
		};
		if d.open.repo != repo || !wants_reload(&d.open.source, changed) {
			return;
		}
		self.diff_serial += 1;
		d.generation = self.diff_serial;
		// While a layout for the other mode is still on its way (the Inline toggle restarted the
		// read), this later read must send rows even for unchanged bytes: it carries the newer
		// generation, and the toggle's result would be dropped (Morticia L-3 on dec2749).
		let known = (d.rows_inline == d.inline).then_some(d.raw_hash).flatten();
		spawn_read(
			d.open.clone(),
			d.generation,
			d.inline,
			known,
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
			d.raw_hash = msg.raw_hash;
		}
	}
}

/// Whether a status read in the diff's repository should re-read it: always for the working
/// tree and the index, whose diffs can change without the status changing; for a commit only
/// when the status changed, since its diff is fixed and only the row's context moved.
fn wants_reload(source: &DiffSource, changed: bool) -> bool {
	match source {
		DiffSource::Working | DiffSource::Staged => true,
		DiffSource::Commit(_) => changed,
	}
}

fn hash_bytes(bytes: &[u8]) -> u64 {
	let mut h = DefaultHasher::new();
	bytes.hash(&mut h);
	h.finish()
}

/// `known` is the hash of the bytes behind the diff already shown; a re-read that gets the same
/// bytes sends nothing, so the rows and colours on screen stand.
fn spawn_read(
	open: OpenDiff,
	generation: u64,
	inline: bool,
	known: Option<u64>,
	tx: Sender<Msg>,
	ctx: egui::Context,
) {
	std::thread::spawn(move || {
		let mut raw_hash = None;
		let content = match largest_side(&open) {
			Ok(size) if size > MAX_DIFF_BYTES => DiffContent::TooLarge(size),
			Ok(_) => match read_diff_bytes(&open.repo, &open.source, &open.paths) {
				Ok(bytes) => {
					let hash = hash_bytes(&bytes);
					if known == Some(hash) {
						return;
					}
					raw_hash = Some(hash);
					DiffContent::Files(parse_unified(&bytes))
				}
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
			raw_hash,
		}));
		ctx.request_repaint();
	});
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Morticia M-1 on 9007389: the status cannot see a second edit to a modified file, so a
	/// working or staged diff must not wait for `changed`; the byte hash is what stops the
	/// needless re-layout.
	#[test]
	fn working_and_staged_diffs_reload_on_every_status_and_bytes_decide_the_rest() {
		assert!(wants_reload(&DiffSource::Working, false));
		assert!(wants_reload(&DiffSource::Staged, false));
		assert!(!wants_reload(&DiffSource::Commit("abc".into()), false));
		assert!(wants_reload(&DiffSource::Commit("abc".into()), true));
		let first = b"diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+b\n";
		let second = b"diff --git a/x b/x\n@@ -1 +1 @@\n-a\n+c\n";
		assert_eq!(hash_bytes(first), hash_bytes(first));
		assert_ne!(
			hash_bytes(first),
			hash_bytes(second),
			"a second edit changes the hash"
		);
	}
}
