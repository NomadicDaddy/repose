//! The configured remotes: names and URLs from `git remote --verbose` (saved config, no
//! network), shown in the repository header, and the https page a URL names, for Copy remote URL
//! and Open remote page. Also the one network action, Fetch, run only when the user asks.

use std::path::Path;

use super::commands::{Cancel, GitError, Verb, run, run_cancellable};

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

/// `git fetch --quiet` from the repository's configured remote, on the user's Fetch only (A1). A
/// sign-in request or a network failure comes back as the error with git's own message; the
/// repository's refs are left as they were. Cancel stops git and anything it started.
pub fn fetch(repo: &Path, cancel: Option<&Cancel>) -> Result<(), GitError> {
	run_cancellable(repo, Verb::Fetch, &[], cancel).map(|_| ())
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
	// The RFC authority first: it ends at the first slash, question mark or hash, and everything
	// up to its last at-sign is userinfo. A query is never searched for an at-sign, so a pathless
	// host.invalid?access_token=abc@secret shows the host (Roger Medium on f6b80e4).
	let authority = rest.find(['/', '?', '#']).map_or(rest, |i| &rest[..i]);
	let after_userinfo = match authority.rfind('@') {
		Some(at) => &rest[at + 1..],
		None => {
			// A password with a raw slash or hash in it (malformed, but git prints it as given)
			// ends that authority early. When the authority has a colon that is not a port, the
			// userinfo ends at the last at-sign of the untrimmed text, up to a query that begins
			// after the path (Morticia Lows on 79ec98d and b5cc402). A path's own at-sign with no
			// userinfo in front stays, a bracketed IPv6 host, colons and all, is never userinfo,
			// and a token-only userinfo holding a raw ? or / (tok?en@host) is the accepted edge:
			// nothing tells it from a host, and git cannot use such a URL either.
			// A host carries a dot before its colon and a username almost never does, so an
			// authority like host.invalid:bad is a host with a bad port, not userinfo, and the
			// guard leaves it alone (Roger Low on 0c3dc68, Morticia #31958). The edge that
			// remains is a dotted username with a raw slash or hash in its password.
			let colon_not_port = !authority.starts_with('[')
				&& authority.split_once(':').is_some_and(|(before, after)| {
					!before.contains('.')
						&& (after.is_empty() || !after.bytes().all(|b| b.is_ascii_digit()))
				});
			let path_start = rest.find('/').unwrap_or(rest.len());
			let query_after_path = rest[path_start..]
				.find(['?', '#'])
				.map_or(rest.len(), |i| path_start + i);
			match rest[..query_after_path].rfind('@') {
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
/// Windows drive or a `file://` URL has no page. A trailing `.git` or `/` is dropped. A scheme
/// URL goes through `display_url` first, so the browser gets no userinfo (a slash-containing
/// password included), query or fragment; an scp path loses its query and fragment too
/// (remote-page-sanitization). Copy remote URL stays the verbatim action.
pub fn https_page(url: &str) -> Option<String> {
	let url = url.trim();
	// Userinfo with a raw ? or # (tok?en@host, first.last:p?w@host, to.k?en@host) is malformed,
	// and display_url keeps only its first piece; opening that as a host would send the browser
	// and DNS to it (Morticia L-1 on baf23c8 and on 6096728). Raw text cannot tell a dotted name
	// from a dotted host, so any at-sign after a raw ? or # before the first slash has no page,
	// a pathless host with an at-sign in its query included (localhost?who=a@b, settled on
	// Roger's Low on a89d417): no page, never a leak.
	if let Some((_, rest)) = url.split_once("://") {
		let raw_authority = rest.split('/').next().unwrap_or(rest);
		if raw_authority
			.find(['?', '#'])
			.is_some_and(|cut| raw_authority[cut..].contains('@'))
		{
			return None;
		}
	}
	let shown = display_url(url);
	if let Some((scheme, rest)) = shown.split_once("://") {
		let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
		let authority = authority
			.rsplit_once('@')
			.map_or(authority, |(_, host)| host);
		if authority.is_empty() {
			return None;
		}
		let (page_scheme, host) = match scheme {
			"https" | "http" => (scheme, authority),
			"ssh" | "git+ssh" | "git" => ("https", ssh_host(authority)?),
			_ => return None,
		};
		return Some(format!("{page_scheme}://{host}/{}", strip_git(path)));
	}
	if url.contains(":\\") || url.starts_with('/') || url.starts_with('.') {
		return None;
	}
	// scp form: [user@]host:path, where a bracketed IPv6 host ends at `]:`.
	let (host_part, path) = scp_split(url)?;
	let path = path.find(['?', '#']).map_or(path, |i| &path[..i]);
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

/// Why a remote URL has no web page, for the menu's disabled Open remote page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoPage {
	/// A path on disk: plain or relative (a bare name included), a Windows drive, or `file://`.
	LocalPath,
	/// A URL `https_page` refuses: an unsupported scheme, or a malformed or unsafe form.
	Refused,
}

/// Why `https_page` gives no page for `url`; asked only after it gave none. A path is told
/// apart by the same rules `https_page` uses to refuse it; anything else, an empty URL
/// included, is a refused URL.
pub fn no_page_reason(url: &str) -> NoPage {
	let url = url.trim();
	if let Some((scheme, _)) = url.split_once("://") {
		return if scheme.eq_ignore_ascii_case("file") {
			NoPage::LocalPath
		} else {
			NoPage::Refused
		};
	}
	let local = !url.is_empty()
		&& (url.contains(":\\")
			|| url.starts_with(['/', '\\', '.'])
			|| url
				.split_once(':')
				.is_none_or(|(before, _)| before.len() == 1 || before.contains('/')));
	if local {
		NoPage::LocalPath
	} else {
		NoPage::Refused
	}
}

/// The host of an ssh-style authority without its port. A bracketed IPv6 address keeps its
/// colons through the closing `]` (`[2001:db8::1]:2222` is `[2001:db8::1]`); an unclosed bracket
/// has no host. Any other authority drops everything from its first colon.
fn ssh_host(authority: &str) -> Option<&str> {
	if authority.starts_with('[') {
		let close = authority.find(']')?;
		return Some(&authority[..=close]);
	}
	Some(authority.split_once(':').map_or(authority, |(h, _)| h))
}

/// An scp-form URL split into its `[user@]host` and its path. A `[` before any colon opens a
/// bracketed IPv6 host (`git@[2001:db8::1]:group/repo.git`), which ends at `]:`; without that
/// there is no host. Any other URL splits at its first colon.
fn scp_split(url: &str) -> Option<(&str, &str)> {
	match url.find('[') {
		Some(open) if !url[..open].contains(':') => {
			let close = open + url[open..].find("]:")?;
			Some((&url[..=close], &url[close + 2..]))
		}
		_ => url.split_once(':'),
	}
}

fn strip_git(path: &str) -> &str {
	let path = path.trim_end_matches('/');
	path.strip_suffix(".git").unwrap_or(path)
}
