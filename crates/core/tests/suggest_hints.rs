//! ai-commit-message-style-hints (A1, Phillip #61712 on #61648, wording #61716): with style hints
//! on, a request also carries the subject lines of the last 10 commits on HEAD, newest first, and
//! nothing more of them. The subjects go before any diff and count inside the 64 KiB cap. A
//! repository with no commits adds none, and nothing unstaged is ever sent.
#![allow(clippy::expect_used)]

mod support;

use repose_core::git::diff::staged_for_suggestion;
use repose_core::git::log::{STYLE_HINTS, recent_subjects};
use repose_core::suggest::{CAP, INSTRUCTION, Staged, request};
use support::TempRepo;

/// A staged diff of one file whose body is `size` bytes of added lines.
fn file_diff(path: &str, size: usize) -> String {
	let mut text = format!(
		"diff --git a/{path} b/{path}\nindex 1111111..2222222 100644\n--- a/{path}\n+++ b/{path}\n@@ -0,0 +1 @@\n"
	);
	while text.len() < size {
		text.push_str("+a staged line\n");
	}
	text
}

/// Twelve commits, each with a body, an author line git records and a trailer: the read keeps the
/// ten newest subjects in order and nothing else of them.
#[test]
fn the_hints_are_the_ten_newest_subjects_and_nothing_else() {
	let repo = TempRepo::new("hints-ten");
	for n in 1..=12 {
		repo.git(&[
			"commit",
			"--allow-empty",
			"-q",
			"-m",
			&format!("subject {n:02}"),
			"-m",
			&format!("BODY-TEXT-{n}"),
			"-m",
			"Signed-off-by: Trailer Person <trailer@example.invalid>",
		]);
	}
	let subjects = recent_subjects(&repo.path).expect("subjects");
	let expected: Vec<String> = (3..=12).rev().map(|n| format!("subject {n:02}")).collect();
	assert_eq!(STYLE_HINTS, 10);
	assert_eq!(subjects, expected);
	let joined = subjects.join("\n");
	assert!(!joined.contains("BODY-TEXT") && !joined.contains("Signed-off-by"));
	assert!(!joined.contains("main"), "no branch name");
}

/// A repository with no commits yet reads as no subjects, not an error, and adds no hints block.
#[test]
fn a_repository_with_no_commits_adds_no_hints() {
	let repo = TempRepo::new("hints-unborn");
	assert_eq!(
		recent_subjects(&repo.path).expect("unborn reads"),
		Vec::<String>::new()
	);
	let small = file_diff("a.txt", 200);
	let built = request(&Staged {
		files: vec![("a.txt".to_string(), 1)],
		diff: small.into_bytes(),
		subjects: Vec::new(),
	})
	.expect("a request");
	assert!(!built.text.contains("Recent commit subjects"));
	assert!(
		built
			.text
			.starts_with(&format!("{INSTRUCTION}\n\nStaged files:\n"))
	);
}

/// The hints sit right after the instruction, before the file list and every diff.
#[test]
fn the_hints_go_before_any_diff() {
	let small = file_diff("a.txt", 200);
	let built = request(&Staged {
		files: vec![("a.txt".to_string(), 1)],
		diff: small.clone().into_bytes(),
		subjects: vec!["fix(app): newest".to_string(), "feat: older".to_string()],
	})
	.expect("a request");
	let hints = built
		.text
		.find("- fix(app): newest\n- feat: older\n")
		.expect("hints");
	assert!(built.text[..hints].starts_with(INSTRUCTION));
	assert!(hints < built.text.find("Staged files:").expect("list"));
	assert!(hints < built.text.find(&small).expect("diff"));
}

/// The hints count inside the cap: a diff that fits exactly without them is left out with them,
/// and the request never grows past the cap's own budget for hints and diffs.
#[test]
fn the_hints_count_inside_the_cap() {
	let subjects = vec!["a subject that takes some room".to_string(); 10];
	let without = request(&Staged {
		files: vec![("a.txt".to_string(), 1)],
		diff: file_diff("a.txt", CAP - 100).into_bytes(),
		subjects: Vec::new(),
	})
	.expect("a request");
	assert!(without.left_out.is_empty(), "it fits on its own");
	let with = request(&Staged {
		files: vec![("a.txt".to_string(), 1)],
		diff: file_diff("a.txt", CAP - 100).into_bytes(),
		subjects,
	})
	.expect("a request");
	assert_eq!(with.left_out, vec!["a.txt"], "the hints take its room");
	assert!(with.text.contains("left out to stay under 64 KiB: a.txt"));
	let huge = vec!["x".repeat(CAP); 2];
	let capped = request(&Staged {
		files: Vec::new(),
		diff: Vec::new(),
		subjects: huge,
	})
	.expect("a request");
	assert!(
		capped.text.len() < CAP + 1024,
		"subjects alone never break the cap"
	);
}

/// With hints on, an unstaged edit and an untracked file still stay out of the request.
#[test]
fn hints_send_nothing_unstaged_or_untracked() {
	let repo = TempRepo::new("hints-unstaged");
	repo.write("staged.txt", "base\n");
	repo.write("edited.txt", "base\n");
	repo.commit_all("first commit");
	repo.write("staged.txt", "STAGED-CHANGE\n");
	repo.git(&["add", "staged.txt"]);
	repo.write("edited.txt", "UNSTAGED-CHANGE\n");
	repo.write("new.txt", "UNTRACKED-FILE\n");
	let (files, diff) = staged_for_suggestion(&repo.path).expect("staged read");
	let subjects = recent_subjects(&repo.path).expect("subjects");
	assert_eq!(subjects, vec!["first commit"]);
	let built = request(&Staged {
		files,
		diff,
		subjects,
	})
	.expect("a request");
	assert!(built.text.contains("STAGED-CHANGE") && built.text.contains("- first commit\n"));
	assert!(!built.text.contains("UNSTAGED-CHANGE") && !built.text.contains("UNTRACKED-FILE"));
}
