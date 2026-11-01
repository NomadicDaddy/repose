//! The Files tab and the file viewer: one committed tree per repository, at HEAD or at the commit
//! chosen in History, read on a background thread when the tab shows; and one file open
//! read-only, its lines coloured on the reader thread the way a diff's are.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use eframe::egui;
use repose_core::git::diff::{MAX_DIFF_BYTES, blob_size};
use repose_core::git::tree::{is_binary, read_file, read_tree};

use super::{AppState, Msg};
use crate::file_tree::{Folder, build_folder_tree};
use crate::highlight::{Span, colour_lines};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FileRev {
	#[default]
	Head,
	Commit(String),
}

impl FileRev {
	/// What git is asked for.
	pub fn git_name(&self) -> &str {
		match self {
			Self::Head => "HEAD",
			Self::Commit(h) => h,
		}
	}

	/// What the pane shows: `HEAD` or the abbreviated hash.
	pub fn label(&self) -> &str {
		match self {
			Self::Head => "HEAD",
			Self::Commit(h) => h.get(..7).unwrap_or(h),
		}
	}
}

#[derive(Default)]
pub struct FilesState {
	pub rev: FileRev,
	/// The revision `tree` was read for; a read starts when it differs from `rev`.
	pub loaded: Option<FileRev>,
	pub loading: bool,
	/// Bumped by every read; a result carrying an older value is dropped.
	pub generation: u64,
	pub tree: Folder,
	pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenFile {
	pub repo: PathBuf,
	pub rev: FileRev,
	pub path: String,
	/// From the tree listing; None when opened from the command line, measured before the read.
	pub size: Option<u64>,
}

pub enum FileContent {
	Loading,
	TooLarge(u64),
	Binary,
	Text {
		lines: Vec<String>,
		colours: Vec<Vec<Span>>,
	},
	Error(String),
}

pub struct FileViewState {
	pub open: OpenFile,
	pub content: FileContent,
	pub generation: u64,
	/// The line last clicked, one-based; Open in VS Code goes to it.
	pub current_line: Option<usize>,
	/// The last external action's failure, shown under the breadcrumb.
	pub notice: Option<String>,
}

pub enum FilesMsg {
	Tree {
		repo: PathBuf,
		generation: u64,
		rev: FileRev,
		result: Result<Folder, String>,
	},
	File {
		generation: u64,
		content: FileContent,
	},
}

impl AppState {
	pub fn files_of(&self, repo: &Path) -> Option<&FilesState> {
		self.files.get(repo)
	}

	/// Makes sure the tree for the repository's current Files revision is loaded or loading.
	pub fn ensure_files(&mut self, repo: &Path, ctx: &egui::Context) {
		let f = self.files.entry(repo.to_path_buf()).or_default();
		if f.loading || f.loaded.as_ref() == Some(&f.rev) {
			return;
		}
		f.loading = true;
		f.generation += 1;
		spawn_tree(
			repo.to_path_buf(),
			f.rev.clone(),
			f.generation,
			self.tx.clone(),
			ctx.clone(),
		);
	}

	/// Points the Files tab at a revision; the tree is read when the tab next shows.
	pub fn set_files_rev(&mut self, repo: &Path, rev: FileRev) {
		self.files.entry(repo.to_path_buf()).or_default().rev = rev;
	}

	/// HEAD moved: a tree read at HEAD is stale, and so is a file open there. A commit's tree
	/// never changes, so those stand.
	pub(super) fn refresh_files_for(&mut self, repo: &Path, ctx: &egui::Context) {
		if let Some(f) = self.files.get_mut(repo)
			&& f.rev == FileRev::Head
		{
			f.loaded = None;
			f.loading = false;
			f.generation += 1;
		}
		if let Some(v) = self.file_view.as_mut()
			&& v.open.repo == repo
			&& v.open.rev == FileRev::Head
		{
			self.file_serial += 1;
			v.generation = self.file_serial;
			// The size came from the old tree; the new HEAD's copy is measured afresh, so a file
			// that grew past the limit is refused and one that shrank opens (Morticia L-1 on
			// e176de4).
			v.open.size = None;
			spawn_file(v.open.clone(), v.generation, self.tx.clone(), ctx.clone());
		}
	}

	/// Opens a file read-only in the right pane, in place of any diff.
	pub fn open_file(&mut self, open: OpenFile, ctx: &egui::Context) {
		if self.file_view.as_ref().is_some_and(|v| v.open == open) {
			return;
		}
		self.close_diff();
		self.file_serial += 1;
		let generation = self.file_serial;
		self.file_view = Some(FileViewState {
			open: open.clone(),
			content: FileContent::Loading,
			generation,
			current_line: None,
			notice: None,
		});
		spawn_file(open, generation, self.tx.clone(), ctx.clone());
	}

	pub fn close_file(&mut self) {
		self.file_view = None;
	}

	pub(super) fn merge_files(&mut self, msg: FilesMsg) {
		match msg {
			FilesMsg::Tree {
				repo,
				generation,
				rev,
				result,
			} => {
				let Some(f) = self.files.get_mut(&repo) else {
					return;
				};
				if generation != f.generation {
					return;
				}
				f.loading = false;
				match result {
					Ok(tree) => {
						f.tree = tree;
						f.error = None;
						f.loaded = Some(rev);
					}
					Err(e) => {
						f.tree = Folder::default();
						f.error = Some(e);
						f.loaded = Some(rev);
					}
				}
			}
			FilesMsg::File {
				generation,
				content,
			} => {
				if let Some(v) = self.file_view.as_mut()
					&& v.generation == generation
				{
					v.content = content;
				}
			}
		}
	}
}

fn spawn_tree(repo: PathBuf, rev: FileRev, generation: u64, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let result = read_tree(&repo, rev.git_name())
			.map(|entries| build_folder_tree(&entries))
			.map_err(|e| e.to_string());
		let _ = tx.send(Msg::Files(FilesMsg::Tree {
			repo,
			generation,
			rev,
			result,
		}));
		ctx.request_repaint();
	});
}

/// Size first (from the listing, or measured when the file came from the command line), then
/// the bytes, then the lines and their colours, all off the UI thread.
fn spawn_file(open: OpenFile, generation: u64, tx: Sender<Msg>, ctx: egui::Context) {
	std::thread::spawn(move || {
		let content = match measured_size(&open) {
			Err(e) => FileContent::Error(e),
			Ok(size) if size > MAX_DIFF_BYTES => FileContent::TooLarge(size),
			Ok(_) => match read_file(&open.repo, open.rev.git_name(), &open.path) {
				Err(e) => FileContent::Error(e.to_string()),
				Ok(bytes) if is_binary(&bytes) => FileContent::Binary,
				Ok(bytes) => {
					let text = String::from_utf8_lossy(&bytes);
					let lines: Vec<String> = text.lines().map(str::to_string).collect();
					let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
					let colours = colour_lines(&open.path, &refs);
					FileContent::Text { lines, colours }
				}
			},
		};
		let _ = tx.send(Msg::Files(FilesMsg::File {
			generation,
			content,
		}));
		ctx.request_repaint();
	});
}

fn measured_size(open: &OpenFile) -> Result<u64, String> {
	if let Some(size) = open.size {
		return Ok(size);
	}
	match blob_size(&open.repo, open.rev.git_name(), &open.path) {
		Ok(Some(n)) => Ok(n),
		Ok(None) => Err(format!("{} is not in {}.", open.path, open.rev.label())),
		Err(e) => Err(e.to_string()),
	}
}
