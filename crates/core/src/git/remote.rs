//! The origin remote, read-only: its configured URL from `git remote get-url origin` (saved
//! config, no network) and the https page that URL names, for Copy remote URL and Open remote
//! page.

use std::path::Path;

use super::commands::{GitError, Verb, run};

/// `Ok(None)` when the repository has no remote named origin: git exits 2 with "No such
/// remote" (exit 128 on older versions, with the same message); any other failure is an error.
pub fn read_origin_url(repo: &Path) -> Result<Option<String>, GitError> {
	match run(repo, Verb::RemoteGetUrl, &["origin"]) {
		Ok(out) => {
			let url = String::from_utf8_lossy(&out).trim().to_string();
			Ok((!url.is_empty()).then_some(url))
		}
		Err(GitError::Failed {
			code: Some(2 | 128),
			ref stderr_tail,
		}) if stderr_tail.contains("No such remote") => Ok(None),
		Err(e) => Err(e),
	}
}

/// The web page for a remote URL, or None when the form is not one a browser can open:
/// `git@github.com:org/repo.git` and `ssh://git@github.com/org/repo.git` become
/// `https://github.com/org/repo`; an `https://` or `http://` URL keeps its scheme and loses a
/// trailing `.git`; a local path or a `file://` URL has no page.
pub fn https_page(url: &str) -> Option<String> {
	let url = url.trim();
	let strip_git = |s: &str| s.trim_end_matches('/').trim_end_matches(".git").to_string();
	if let Some(rest) = url
		.strip_prefix("https://")
		.or_else(|| url.strip_prefix("http://"))
	{
		let scheme = &url[..url.len() - rest.len()];
		let rest = rest.split_once('@').map_or(rest, |(_, r)| r);
		return Some(format!("{scheme}{}", strip_git(rest)));
	}
	if let Some(rest) = url
		.strip_prefix("ssh://")
		.or_else(|| url.strip_prefix("git+ssh://"))
	{
		let rest = rest.split_once('@').map_or(rest, |(_, r)| r);
		let (host, path) = rest.split_once('/')?;
		let host = host.split_once(':').map_or(host, |(h, _)| h);
		return Some(format!("https://{host}/{}", strip_git(path)));
	}
	if url.starts_with("file://") || url.contains(":\\") || url.starts_with('/') {
		return None;
	}
	// scp form: [user@]host:path
	let (host_part, path) = url.split_once(':')?;
	let host = host_part.split_once('@').map_or(host_part, |(_, h)| h);
	// A one-letter "host" is a Windows drive (C:/repos/x), which git also reads as a path.
	if host.is_empty() || path.is_empty() || host.contains('/') || host.len() == 1 {
		return None;
	}
	Some(format!(
		"https://{host}/{}",
		strip_git(path.trim_start_matches('/'))
	))
}
