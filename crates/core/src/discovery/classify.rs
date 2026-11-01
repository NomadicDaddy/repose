//! Tells a repository from a worktree from a submodule by reading `.git`, `HEAD`, refs and
//! `.gitmodules`. No git process is spawned here.

use std::fs;
use std::path::{Path, PathBuf};

use crate::model::{BranchState, RepoKind, RepoSummary};

/// What the walk found at one directory.
#[derive(Debug, Clone)]
pub enum Found {
	Repository(RepoSummary),
	Worktree { main: PathBuf, summary: RepoSummary },
}

pub fn classify(dir: &Path) -> Option<Found> {
	let dot = dir.join(".git");
	if dot.is_dir() {
		let mut summary = RepoSummary::new(dir.to_path_buf(), RepoKind::Repository);
		read_head(&dot, &dot, &mut summary);
		summary.submodules = submodule_count(dir);
		return Some(Found::Repository(summary));
	}
	if !dot.is_file() {
		return None;
	}
	let text = fs::read_to_string(&dot).ok()?;
	let gitdir = resolve_gitdir(dir, text.trim().strip_prefix("gitdir:")?.trim());
	if is_submodule_gitdir(&gitdir) {
		// A submodule checkout: counted on its parent, never a row of its own.
		return None;
	}
	if let Some(main) = worktree_main(&gitdir) {
		let mut summary = RepoSummary::new(dir.to_path_buf(), RepoKind::Worktree);
		summary.main_path = Some(main.clone());
		read_head(&gitdir, &main.join(".git"), &mut summary);
		return Some(Found::Worktree { main, summary });
	}
	let mut summary = RepoSummary::new(dir.to_path_buf(), RepoKind::Repository);
	read_head(&gitdir, &gitdir, &mut summary);
	summary.submodules = submodule_count(dir);
	Some(Found::Repository(summary))
}

fn resolve_gitdir(dir: &Path, raw: &str) -> PathBuf {
	let p = PathBuf::from(raw);
	if p.is_absolute() { p } else { dir.join(p) }
}

/// A submodule's `.git` file points into its parent's `.git/modules/<path>`.
fn is_submodule_gitdir(gitdir: &Path) -> bool {
	let text = gitdir.to_string_lossy().replace('\\', "/");
	text.contains("/.git/modules/")
}

/// `<main>/.git/worktrees/<name>` gives `<main>`; anything else is not a linked worktree.
pub fn worktree_main(gitdir: &Path) -> Option<PathBuf> {
	let worktrees = gitdir.parent()?;
	if worktrees.file_name()?.to_string_lossy() != "worktrees" {
		return None;
	}
	let dot_git = worktrees.parent()?;
	if dot_git.file_name()?.to_string_lossy() != ".git" {
		return None;
	}
	dot_git.parent().map(Path::to_path_buf)
}

/// Branch and short hash from `HEAD`, loose refs and `packed-refs`, without git.
fn read_head(git_dir: &Path, common: &Path, summary: &mut RepoSummary) {
	let head = fs::read_to_string(git_dir.join("HEAD")).unwrap_or_default();
	let head = head.trim();
	if let Some(reference) = head.strip_prefix("ref: ") {
		let name = reference.strip_prefix("refs/heads/").unwrap_or(reference);
		summary.branch = BranchState::Branch(name.to_string());
		summary.short_hash = ref_hash(common, reference).map(|h| short(&h));
		if summary.short_hash.is_none() {
			summary.branch = BranchState::Unborn;
		}
		return;
	}
	if head.len() >= 7 {
		summary.branch = BranchState::Detached;
		summary.short_hash = Some(short(head));
	}
}

fn ref_hash(common: &Path, reference: &str) -> Option<String> {
	if let Ok(text) = fs::read_to_string(common.join(reference)) {
		let t = text.trim();
		if t.len() >= 7 {
			return Some(t.to_string());
		}
	}
	let packed = fs::read_to_string(common.join("packed-refs")).ok()?;
	packed.lines().find_map(|line| {
		let (hash, name) = line.split_once(' ')?;
		(name == reference).then(|| hash.to_string())
	})
}

fn short(hash: &str) -> String {
	hash.chars().take(7).collect()
}

fn submodule_count(dir: &Path) -> usize {
	fs::read_to_string(dir.join(".gitmodules"))
		.map(|t| {
			t.lines()
				.filter(|l| l.trim_start().starts_with("[submodule"))
				.count()
		})
		.unwrap_or(0)
}
