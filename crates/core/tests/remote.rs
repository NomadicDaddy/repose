//! The remotes reader against real git (no remote, added remotes, a differing push URL), its
//! parser on git's verbose listing, and the page conversion.
#![allow(clippy::expect_used)]

mod support;

use repose_core::git::remote::{
	RemoteEntry, display_url, https_page, origin, parse_remotes, read_remotes,
};
use support::TempRepo;

fn entry(name: &str, url: &str, push_url: Option<&str>) -> RemoteEntry {
	RemoteEntry {
		name: name.to_string(),
		url: url.to_string(),
		push_url: push_url.map(str::to_string),
	}
}

#[test]
fn remotes_are_empty_without_one_and_listed_in_order_with_a_differing_push_url() {
	let repo = TempRepo::new("remote");
	assert_eq!(
		read_remotes(&repo.path).expect("no remote is not an error"),
		Vec::new()
	);
	repo.git(&[
		"remote",
		"add",
		"origin",
		"git@github.com:example/thing.git",
	]);
	repo.git(&["remote", "add", "upstream", "https://example.invalid/x.git"]);
	let listed = read_remotes(&repo.path).expect("two remotes");
	assert_eq!(
		listed,
		vec![
			entry("origin", "git@github.com:example/thing.git", None),
			entry("upstream", "https://example.invalid/x.git", None),
		]
	);
	assert_eq!(
		origin(&listed).map(|r| r.url.as_str()),
		Some("git@github.com:example/thing.git")
	);
	repo.git(&[
		"remote",
		"set-url",
		"--push",
		"upstream",
		"ssh://push.example.invalid/x",
	]);
	assert_eq!(
		read_remotes(&repo.path).expect("push url")[1],
		entry(
			"upstream",
			"https://example.invalid/x.git",
			Some("ssh://push.example.invalid/x")
		)
	);
}

/// The header never paints a token kept in a remote URL (Morticia and Roger on 462cf80).
#[test]
fn a_displayed_url_loses_its_userinfo_and_keeps_an_scp_login() {
	assert_eq!(
		display_url("https://user:ghp_secret@github.com/team/thing.git"),
		"https://github.com/team/thing.git"
	);
	assert_eq!(
		display_url("https://token@example.invalid/x"),
		"https://example.invalid/x"
	);
	assert_eq!(
		display_url("ssh://git@host.invalid:2222/r"),
		"ssh://host.invalid:2222/r"
	);
	assert_eq!(
		display_url("https://plain.invalid/r.git"),
		"https://plain.invalid/r.git"
	);
	assert_eq!(
		display_url("https://user:pw@host.invalid"),
		"https://host.invalid"
	);
	assert_eq!(
		display_url("git@github.com:team/thing.git"),
		"git@github.com:team/thing.git"
	);
	assert_eq!(display_url("C:\\repos\\thing"), "C:\\repos\\thing");
	// A raw slash inside the password (Morticia Low on 79ec98d) still loses the userinfo, while
	// a path's own at-sign behind a host with a port stays.
	assert_eq!(
		display_url("https://user:pa/ss@host.invalid/x"),
		"https://host.invalid/x"
	);
	assert_eq!(
		display_url("https://host.invalid:8443/@scope/pkg"),
		"https://host.invalid:8443/@scope/pkg"
	);
	assert_eq!(
		display_url("https://user:pa/ss@host.invalid/x?token=t"),
		"https://host.invalid/x"
	);
	// A query or a fragment never shows: a token can ride in a query (Roger on 79ec98d).
	assert_eq!(
		display_url("https://host.invalid/repo.git?access_token=example_secret"),
		"https://host.invalid/repo.git"
	);
	assert_eq!(
		display_url("https://host.invalid/repo.git#private_token=x"),
		"https://host.invalid/repo.git"
	);
	// A raw # or ? inside a malformed password must not end the string before its at-sign
	// (Morticia Low on b5cc402): the userinfo goes first, the query and fragment after.
	assert_eq!(
		display_url("https://user:p#ss@host.invalid/x"),
		"https://host.invalid/x"
	);
	// The token-only userinfo with a raw ? is the accepted edge: nothing tells it from a host.
	assert_eq!(display_url("https://tok?en@host.invalid/x"), "https://tok");
	// A pathless URL whose query holds an at-sign keeps its host and shows no query
	// (Roger Medium on f6b80e4); a query behind a path never feeds the malformed guard.
	assert_eq!(
		display_url("https://host.invalid?access_token=abc@secret"),
		"https://host.invalid"
	);
	assert_eq!(
		display_url("https://user:pa/ss@host.invalid/x?who=a@b"),
		"https://host.invalid/x"
	);
	assert_eq!(
		display_url("https://host.invalid/x?who=a@b"),
		"https://host.invalid/x"
	);
	// A dotted authority before its colon is a host with a bad port, never userinfo, so the
	// guard stays out of its query (Roger Low on 0c3dc68); the pathless raw-hash password still
	// loses its userinfo; the edge left is a dotted username with a raw slash in its password.
	assert_eq!(
		display_url("https://host.invalid:bad?access_token=abc@secret"),
		"https://host.invalid:bad"
	);
	assert_eq!(
		display_url("https://user:p#ss@host.invalid"),
		"https://host.invalid"
	);
	assert_eq!(
		display_url("https://first.last:p/w@host.invalid/x"),
		"https://first.last:p/w@host.invalid/x"
	);
	// A bracketed IPv6 host is never userinfo, so an at-sign in its path stays (Morticia Info).
	assert_eq!(
		display_url("ssh://[::1]:2222/@scope/pkg"),
		"ssh://[::1]:2222/@scope/pkg"
	);
}

#[test]
fn the_verbose_listing_parses_pairs_and_push_only_lines() {
	let out = b"origin\thttps://a.invalid/r.git (fetch)\norigin\thttps://a.invalid/r.git (push)\n\
mirror\tgit@b.invalid:r.git (fetch)\nmirror\tssh://c.invalid/r (push)\nbare\tfile:///tmp/r (push)\n";
	assert_eq!(
		parse_remotes(out),
		vec![
			entry("origin", "https://a.invalid/r.git", None),
			entry("mirror", "git@b.invalid:r.git", Some("ssh://c.invalid/r")),
			entry("bare", "file:///tmp/r", None),
		]
	);
	assert_eq!(parse_remotes(b""), Vec::new());
	assert!(origin(&parse_remotes(b"mirror\tx (fetch)\n")).is_none());
}

#[test]
fn remote_urls_become_pages() {
	for (url, page) in [
		(
			"git@github.com:example/thing.git",
			Some("https://github.com/example/thing"),
		),
		(
			"git@github.com:example/thing",
			Some("https://github.com/example/thing"),
		),
		(
			"ssh://git@github.com/example/thing.git",
			Some("https://github.com/example/thing"),
		),
		(
			"ssh://git@host.example:2222/group/sub/thing.git",
			Some("https://host.example/group/sub/thing"),
		),
		(
			"https://github.com/example/thing.git",
			Some("https://github.com/example/thing"),
		),
		(
			"https://user@dev.azure.com/org/project/_git/thing",
			Some("https://dev.azure.com/org/project/_git/thing"),
		),
		(
			"http://gitea.local/example/thing.git/",
			Some("http://gitea.local/example/thing"),
		),
		(
			"https://github.com/org/name@team/repo.git",
			Some("https://github.com/org/name@team/repo"),
		),
		(
			"https://github.com/a/b@evil.example",
			Some("https://github.com/a/b@evil.example"),
		),
		(
			"ssh://git@github.com/x/y@evil.example.git",
			Some("https://github.com/x/y@evil.example"),
		),
		("git://github.com/x/y.git", Some("https://github.com/x/y")),
		(
			"https://host.example:8443/org/repo.git",
			Some("https://host.example:8443/org/repo"),
		),
		(
			"user@host.example:org/na@me.git",
			Some("https://host.example/org/na@me"),
		),
		// remote-page-sanitization: no query, fragment or userinfo reaches the browser.
		(
			"https://host.example/org/repo.git?access_token=synthetic",
			Some("https://host.example/org/repo"),
		),
		(
			"https://host.example/org/repo.git#private_token=synthetic",
			Some("https://host.example/org/repo"),
		),
		(
			"https://user:pa/ss@host.example/org/repo.git",
			Some("https://host.example/org/repo"),
		),
		(
			"git@host.example:org/repo.git?token=synthetic",
			Some("https://host.example/org/repo"),
		),
		(
			"git@host.example:org/repo.git#frag",
			Some("https://host.example/org/repo"),
		),
		("svn://host.example/x", None),
		("../relative/repo", None),
		("D:\\applications\\thing", None),
		("C:/repos/thing", None),
		("c:/repos/thing.git", None),
		("file:///D:/applications/thing", None),
		("/srv/git/thing.git", None),
		("", None),
	] {
		assert_eq!(https_page(url).as_deref(), page, "{url}");
	}
}
