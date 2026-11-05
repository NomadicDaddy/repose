//! Why a remote URL has no page: a path on disk, or a URL that https_page refuses. The menu shows
//! a different reason for each (remediation-20261104-remote-page-refusal-tooltip).

use repose_core::git::remote::{NoPage, https_page, no_page_reason};

#[test]
fn a_url_with_no_page_says_whether_it_is_a_path_or_a_refused_url() {
	for (url, reason) in [
		("../relative/repo", NoPage::LocalPath),
		("./repo", NoPage::LocalPath),
		("thing", NoPage::LocalPath),
		(r"D:\applications\thing", NoPage::LocalPath),
		(r"\\server\share\thing", NoPage::LocalPath),
		("C:/repos/thing", NoPage::LocalPath),
		("c:/repos/thing.git", NoPage::LocalPath),
		("/srv/git/thing.git", NoPage::LocalPath),
		("some/dir:with-colon", NoPage::LocalPath),
		("file:///D:/applications/thing", NoPage::LocalPath),
		("FILE:///srv/thing", NoPage::LocalPath),
		("http://localhost?who=a@b", NoPage::Refused),
		("http://localhost:8080?who=a@b", NoPage::Refused),
		("https://tok?en@host.invalid/x", NoPage::Refused),
		("svn://host.example/x", NoPage::Refused),
		("ssh://git@[2001:db8::1/repo.git", NoPage::Refused),
		("git@[2001:db8::1:group/repo.git", NoPage::Refused),
		("host.example:", NoPage::Refused),
		("", NoPage::Refused),
	] {
		assert_eq!(https_page(url), None, "{url} has no page");
		assert_eq!(no_page_reason(url), reason, "{url}");
	}
}
