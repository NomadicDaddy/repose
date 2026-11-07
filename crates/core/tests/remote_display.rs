//! The header's shown remote URL for scheme-less remotes: an scp-form URL loses a query or
//! fragment that can carry a token, while a local path, where `#` and `?` are legal in a folder
//! name, stays whole (SECRET_HANDLING_RETENTION finding on a65c541, Morticia #37950).

use repose_core::git::remote::{NoPage, display_url, no_page_reason};

#[test]
fn an_scp_remote_loses_a_token_in_its_query_or_fragment() {
	let token = "SENTINEL-TOKEN-0123";
	let cases = [
		(
			format!("git@host.invalid:org/repo.git?access_token={token}"),
			"git@host.invalid:org/repo.git",
		),
		(
			format!("host.invalid:org/repo.git#private_token={token}"),
			"host.invalid:org/repo.git",
		),
		(
			format!("deploy@git.host.invalid:team/push.git?private_token={token}#x"),
			"deploy@git.host.invalid:team/push.git",
		),
		(
			format!("git@[2001:db8::1]:group/repo.git?token={token}"),
			"git@[2001:db8::1]:group/repo.git",
		),
	];
	for (url, shown) in &cases {
		let got = display_url(url);
		assert_eq!(got, *shown, "{url}");
		assert!(!got.contains(token), "{url} kept its token");
	}
}

#[test]
fn a_local_path_with_a_hash_or_question_mark_stays_whole() {
	let paths = [
		"D:/repos/a#b",
		"D:\\repos\\a#b\\thing",
		"/srv/a#b.git",
		"./mirror?old",
		"repos/a#b",
		"/srv/git/what?.git",
	];
	for path in paths {
		assert_eq!(display_url(path), path);
		// The same rule decides the menu's reason, so the two never disagree on a path.
		assert_eq!(no_page_reason(path), NoPage::LocalPath, "{path}");
	}
}

#[test]
fn an_scp_remote_without_a_query_is_unchanged() {
	for url in [
		"git@github.com:team/thing.git",
		"host.invalid:org/repo",
		"git@[2001:db8::1]:group/repo.git",
	] {
		assert_eq!(display_url(url), url);
	}
}
