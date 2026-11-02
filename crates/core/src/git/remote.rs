//! The configured remotes, read-only: names and URLs from `git remote --verbose` (saved config,
//! no network), shown in the repository header, and the https page a URL names, for Copy remote
//! URL and Open remote page.

use std::path::Path;

use super::commands::{GitError, Verb, run};

/// One configured remote. `push_url` is set only when it differs from the fetch URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEntry {
	pub name: String,
	pub url: String,
	pub push_url: Option<String>,
}

/// Every configured remote in git's order; an empty list when none is configured (git prints
/// nothing and exits 0), so only a failed spawn or a broken repository is an error.
pub fn read_remotes(repo: &Path) -> Result<Vec<RemoteEntry>, GitError> {
	Ok(parse_remotes(&run(repo, Verb::RemoteList, &[])?))
}

/// `git remote --verbose` prints `<name>\t<url> (fetch)` and `<name>\t<url> (push)` per remote.
/// A remote whose push URL equals its fetch URL keeps `push_url` None; a push-only line (a
/// `pushurl` with no `url`) still names the remote, with that URL as its URL.
pub fn parse_remotes(out: &[u8]) -> Vec<RemoteEntry> {
	let mut remotes: Vec<RemoteEntry> = Vec::new();
	for line in String::from_utf8_lossy(out).lines() {
		let Some((name, rest)) = line.split_once('\t') else {
			continue;
		};
		let (url, kind) = rest.rsplit_once(" (").unwrap_or((rest, "fetch)"));
		let is_push = kind.starts_with("push");
		match remotes.iter_mut().find(|r| r.name == name) {
			Some(entry) => {
				if is_push && entry.url != url {
					entry.push_url = Some(url.to_string());
				}
			}
			None => remotes.push(RemoteEntry {
				name: name.to_string(),
				url: url.to_string(),
				push_url: None,
			}),
		}
	}
	remotes
}

/// The remote named origin, which the menu's Copy remote URL and Open remote page use.
pub fn origin(remotes: &[RemoteEntry]) -> Option<&RemoteEntry> {
	remotes.iter().find(|r| r.name == "origin")
}

/// A remote URL as the header may paint it: `scheme://user:token@host/path` loses everything
/// before the `@` in its authority, because git keeps a configured token there verbatim and a
/// selected repository must not put it on screen or into a screenshot (Morticia and Roger on
/// 462cf80). The scp form `git@host:path` keeps its user, which is a login name, not a secret.
/// Copy remote URL still hands over the verbatim URL behind its explicit action.
pub fn display_url(url: &str) -> String {
	let Some((scheme, rest)) = url.split_once("://") else {
		return url.to_string();
	};
	// The userinfo goes first, on the untrimmed text, so a raw ? or # inside a malformed
	// password cannot end the string before its at-sign (Morticia Low on b5cc402). The authority
	// runs to the first slash; everything up to its last at-sign is userinfo.
	let authority = rest.split_once('/').map_or(rest, |(a, _)| a);
	let after_userinfo = match authority.rfind('@') {
		Some(at) => &rest[at + 1..],
		None => {
			// A password with a raw slash in it (malformed, but git prints it as given) puts the
			// first slash inside the userinfo. When what precedes that slash has a colon that is
			// not a port and an at-sign still follows before any query or fragment, the userinfo
			// ends at that at-sign (Morticia Low on 79ec98d). A path's own at-sign with no
			// userinfo in front stays, and a bracketed IPv6 host, colons and all, is never
			// userinfo (Morticia Info on b5cc402).
			let colon_not_port = !authority.starts_with('[')
				&& authority.split_once(':').is_some_and(|(_, after)| {
					after.is_empty() || !after.bytes().all(|b| b.is_ascii_digit())
				});
			let before_query = rest.find(['?', '#']).map_or(rest, |i| &rest[..i]);
			match before_query.rfind('@') {
				Some(at) if colon_not_port => &rest[at + 1..],
				_ => rest,
			}
		}
	};
	// Neither a query nor a fragment identifies a remote, and a query can carry a token
	// (Roger on 79ec98d), so the shown URL ends where either begins.
	let shown = after_userinfo
		.find(['?', '#'])
		.map_or(after_userinfo, |i| &after_userinfo[..i]);
	format!("{scheme}://{shown}")
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
