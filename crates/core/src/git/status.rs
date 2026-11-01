//! One repository's status through `git status --porcelain=v2 -z --branch --untracked-files=all`.

use std::path::Path;
use std::time::SystemTime;

use super::commands::{GitError, Verb, run};
use super::parse::{Porcelain, parse_porcelain_v2};
use crate::model::RepoSummary;

pub const STATUS_ARGS: [&str; 4] = ["--porcelain=v2", "-z", "--branch", "--untracked-files=all"];

pub fn read_status(repo: &Path) -> Result<Porcelain, GitError> {
	let bytes = run(repo, Verb::Status, &STATUS_ARGS)?;
	Ok(parse_porcelain_v2(&bytes))
}

/// Fills a summary from git. An error is recorded on the row and never removes it.
pub fn read_status_into(summary: &mut RepoSummary) {
	summary.checked_at = Some(SystemTime::now());
	match read_status(&summary.path) {
		Ok(p) => {
			summary.branch = p.branch_state();
			if let Some(h) = p.short_hash() {
				summary.short_hash = Some(h);
			}
			summary.status = Some(p.summary());
			summary.error = None;
		}
		Err(e) => {
			summary.error = Some(e.to_string());
		}
	}
}
