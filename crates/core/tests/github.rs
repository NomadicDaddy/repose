//! remote-visibility's gh call (A1): which remotes name a GitHub repository, the exact argument
//! list and environment, how gh's answers map to Public, Private or Unknown, and a stand-in gh run
//! end to end. Nothing here runs the real gh, so the gate never contacts GitHub.
#![allow(clippy::expect_used)]

use std::ffi::OsStr;

use repose_core::host::github::{
	NO_GH, NOT_GITHUB, Visibility, answer, check_with, command, repository,
};

#[test]
fn only_github_https_and_scp_remotes_name_a_repository() {
	let names = |o: &str, r: &str| Ok((o.to_string(), r.to_string()));
	for (url, want) in [
		("https://github.com/team/thing.git", names("team", "thing")),
		("https://github.com/team/thing", names("team", "thing")),
		("https://GitHub.com/team/thing/", names("team", "thing")),
		(
			"https://user:secret@github.com/team/thing.git",
			names("team", "thing"),
		),
		(
			"https://github.com/team/thing?tab=x#y",
			names("team", "thing"),
		),
		("git@github.com:team/thing.git", names("team", "thing")),
		("github.com:team/thing", names("team", "thing")),
		("git@github.com:team/.github.git", names("team", ".github")),
		(
			"git@github.com:my-org/my_repo.v2",
			names("my-org", "my_repo.v2"),
		),
	] {
		assert_eq!(repository(url), want, "{url}");
	}
	for url in [
		"ssh://git@github.com/team/thing.git",
		"http://github.com/team/thing",
		"git://github.com/team/thing",
		"https://gitlab.com/team/thing",
		"https://github.com.example.invalid/team/thing",
		"https://github.com:8443/team/thing",
		"https://github.com/team",
		"https://github.com/team/thing/tree/main",
		"git@gitlab.com:team/thing.git",
		"D:\\applications\\thing",
		"../thing",
		"",
	] {
		assert_eq!(repository(url), Err(NOT_GITHUB), "{url}");
	}
	for url in [
		"git@github.com:-team/thing",
		"git@github.com:team/--jq",
		"git@github.com:../thing",
		"git@github.com:team/..",
		"git@github.com:team/thing;rm",
		"git@github.com:team/thing?access_token=abc",
		"git@github.com:te am/thing",
		"https://github.com/team/%2e%2e",
		"https://github.com//thing",
	] {
		let refused = repository(url);
		assert!(
			refused.is_err() && refused != Err(NOT_GITHUB),
			"{url} is refused as an unsafe name: {refused:?}"
		);
	}
}

#[test]
fn the_gh_call_is_one_get_of_the_visibility_field_with_no_prompt_or_update_check() {
	let cmd = command(OsStr::new("gh"), "team", "thing");
	assert_eq!(cmd.get_program(), "gh");
	let args: Vec<_> = cmd.get_args().collect();
	assert_eq!(
		args,
		[
			"api",
			"--hostname",
			"github.com",
			"--method",
			"GET",
			"repos/team/thing",
			"--jq",
			".visibility"
		]
	);
	let mut envs: Vec<_> = cmd.get_envs().collect();
	envs.sort();
	let one = Some(OsStr::new("1"));
	assert_eq!(
		envs,
		[
			(OsStr::new("GH_NO_EXTENSION_UPDATE_NOTIFIER"), one),
			(OsStr::new("GH_NO_UPDATE_NOTIFIER"), one),
			(OsStr::new("GH_PROMPT_DISABLED"), one),
		],
		"exactly these three, and no token"
	);
}

#[test]
fn gh_answers_map_to_public_private_or_unknown_with_the_reason() {
	assert_eq!(answer(Some(0), b"public\n", b""), Visibility::Public);
	assert_eq!(answer(Some(0), b"private\r\n", b""), Visibility::Private);
	let unknown = |v: Visibility| match v {
		Visibility::Unknown(why) => why,
		other => panic!("expected Unknown, got {other:?}"),
	};
	assert!(unknown(answer(Some(0), b"internal\n", b"")).contains("internal"));
	assert!(unknown(answer(Some(0), b"", b"")).contains("neither public nor private"));
	assert_eq!(
		unknown(answer(Some(1), b"", b"gh: Not Found (HTTP 404)\n")),
		"gh: Not Found (HTTP 404)"
	);
	assert_eq!(
		unknown(answer(
			Some(4),
			b"",
			b"To get started with GitHub CLI, please run:  gh auth login\n\n"
		)),
		"gh: To get started with GitHub CLI, please run:  gh auth login"
	);
	assert!(unknown(answer(Some(1), b"", b"")).contains("exit code 1"));
	assert!(unknown(answer(None, b"", b"")).contains("no message"));
}

#[test]
fn a_remote_that_is_not_github_runs_nothing_and_a_missing_gh_says_so() {
	let missing = OsStr::new("repose-test-no-such-gh-program");
	// Had it spawned, the missing program would answer NO_GH; NOT_GITHUB means nothing ran.
	assert_eq!(
		check_with(missing, "https://example.invalid/team/thing.git"),
		Visibility::Unknown(NOT_GITHUB.to_string())
	);
	assert_eq!(
		check_with(missing, "git@github.com:team/thing.git"),
		Visibility::Unknown(NO_GH.to_string())
	);
}

/// A stand-in gh (a .cmd script) records the arguments and the three settings it was given, and
/// answers like gh would.
#[cfg(windows)]
#[test]
fn a_stand_in_gh_gets_the_exact_call_and_its_answer_or_error_comes_back() {
	let dir = std::env::temp_dir().join(format!("repose-test-gh-{}", std::process::id()));
	std::fs::create_dir_all(&dir).expect("temp dir");
	let script = |name: &str, body: &str| {
		let path = dir.join(name);
		std::fs::write(&path, body.replace('\n', "\r\n")).expect("stand-in gh");
		path
	};
	let ok = script(
		"gh-private.cmd",
		"@echo off\n\
		 >\"%~dp0args.txt\" echo %*\n\
		 >\"%~dp0env.txt\" echo %GH_PROMPT_DISABLED% %GH_NO_UPDATE_NOTIFIER% %GH_NO_EXTENSION_UPDATE_NOTIFIER%\n\
		 echo private\n",
	);
	assert_eq!(
		check_with(ok.as_os_str(), "https://github.com/team/thing.git"),
		Visibility::Private
	);
	let read = |name: &str| {
		std::fs::read_to_string(dir.join(name))
			.expect("the stand-in ran")
			.trim()
			.to_string()
	};
	assert_eq!(
		read("args.txt"),
		"api --hostname github.com --method GET repos/team/thing --jq .visibility"
	);
	assert_eq!(read("env.txt"), "1 1 1");
	let missing = script(
		"gh-404.cmd",
		"@echo off\n>&2 echo gh: Not Found (HTTP 404)\nexit /b 1\n",
	);
	assert_eq!(
		check_with(missing.as_os_str(), "git@github.com:team/gone.git"),
		Visibility::Unknown("gh: Not Found (HTTP 404)".to_string())
	);
	let _ = std::fs::remove_dir_all(&dir);
}
