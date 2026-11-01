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

/// The web page for a remote URL, or None when the form is not one a browser can open.
/// `scheme://[user@]host[:port]/path`: http and https keep their scheme, host and port; ssh,
/// git+ssh and git become https on the host alone; the scp form `[user@]host:path` becomes
/// https on the host. User info is only ever removed from the authority, never from the path,
/// so a path that contains `@` keeps it (Roger 5Q-1, Morticia L-1 on 85ec3b0). A local path, a
/// Windows drive or a `file://` URL has no page. A trailing `.git` or `/` is dropped.
pub fn https_page(url: &str) -> Option<String> {
	let url = url.trim();
	if let Some((scheme, rest)) = url.split_once("://") {
		let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
		let authority = authority
			.rsplit_once('@')
			.map_or(authority, |(_, host)| host);
		if authority.is_empty() {
			return None;
		}
		let (page_scheme, host) = match scheme {
			"https" | "http" => (scheme, authority),
			"ssh" | "git+ssh" | "git" => (
				"https",
				authority.split_once(':').map_or(authority, |(h, _)| h),
			),
			_ => return None,
		};
		return Some(format!("{page_scheme}://{host}/{}", strip_git(path)));
	}
	if url.contains(":\\") || url.starts_with('/') || url.starts_with('.') {
		return None;
	}
	// scp form: [user@]host:path
	let (host_part, path) = url.split_once(':')?;
	let host = host_part.rsplit_once('@').map_or(host_part, |(_, h)| h);
	// A one-letter "host" is a Windows drive (C:/repos/x), which git also reads as a path.
	if host.is_empty() || path.is_empty() || host.contains('/') || host.len() == 1 {
		return None;
	}
	Some(format!(
		"https://{host}/{}",
		strip_git(path.trim_start_matches('/'))
	))
}

fn strip_git(path: &str) -> &str {
	let path = path.trim_end_matches('/');
	path.strip_suffix(".git").unwrap_or(path)
}
