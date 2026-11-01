//! The origin reader against real git (no remote, an added remote) and the page conversion.
#![allow(clippy::expect_used)]

mod support;

use repose_core::git::remote::{https_page, read_origin_url};
use support::TempRepo;

#[test]
fn origin_is_none_without_a_remote_and_its_url_with_one() {
	let repo = TempRepo::new("remote");
	assert_eq!(
		read_origin_url(&repo.path).expect("no remote is not an error"),
		None
	);
	repo.git(&[
		"remote",
		"add",
		"origin",
		"git@github.com:example/thing.git",
	]);
	assert_eq!(
		read_origin_url(&repo.path).expect("origin"),
		Some("git@github.com:example/thing.git".to_string())
	);
	repo.git(&["remote", "add", "upstream", "https://example.invalid/x.git"]);
	assert_eq!(
		read_origin_url(&repo.path).expect("origin, not upstream"),
		Some("git@github.com:example/thing.git".to_string())
	);
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
		("D:\\applications\\thing", None),
		("file:///D:/applications/thing", None),
		("/srv/git/thing.git", None),
		("", None),
	] {
		assert_eq!(https_page(url).as_deref(), page, "{url}");
	}
}
