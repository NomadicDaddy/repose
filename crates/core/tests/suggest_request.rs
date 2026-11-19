//! ai-commit-message-suggestions: what a request carries. Every staged file is listed by git's own
//! name for it, read from real git output (a space before `b/`, a name git quotes, a rename),
//! whole-file diffs follow up to the 64 KiB cap, and a diff that does not line up with the list is
//! refused rather than guessed at (Roger 5Q-2 on d3da4dd).
#![allow(clippy::expect_used)]

mod support;

use repose_core::git::diff::{read_diff_bytes, staged_files, staged_labels};
use repose_core::model::DiffSource;
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

fn staged(files: &[&str], diff: &str) -> Staged {
	Staged {
		files: files.iter().map(|f| f.to_string()).collect(),
		diff: diff.as_bytes().to_vec(),
	}
}

#[test]
fn a_request_lists_every_staged_file_and_whole_diffs_up_to_the_cap() {
	let small = file_diff("src/a.rs", 2_000);
	let big = file_diff("assets/big.json", CAP);
	let after = file_diff("src/b.rs", 1_000);
	let files = ["src/a.rs", "assets/big.json", "src/b.rs"];
	let built = request(&staged(&files, &format!("{small}{big}{after}"))).expect("a request");
	assert!(built.text.starts_with(INSTRUCTION));
	for path in files {
		assert!(
			built.text.contains(&format!("- {path}\n")),
			"{path} is listed"
		);
	}
	// The big file does not fit, and nothing after it is sent either, so git's order is kept.
	assert_eq!(built.left_out, vec!["assets/big.json", "src/b.rs"]);
	assert!(
		built.text.contains(&small),
		"a diff that fits is sent whole"
	);
	assert!(
		!built.text.contains("+++ b/assets/big.json"),
		"a left-out diff is not cut part-way"
	);
	assert!(!built.text.contains("+++ b/src/b.rs"));
	assert!(
		built
			.text
			.contains("left out to stay under 64 KiB: assets/big.json, src/b.rs")
	);
	let whole = request(&staged(&["src/a.rs"], &small)).expect("a request");
	assert!(whole.left_out.is_empty());
	assert!(!whole.text.contains("left out"));
}

#[test]
fn a_diff_that_does_not_line_up_with_the_list_is_refused() {
	let one = file_diff("a.txt", 200);
	assert_eq!(
		request(&staged(&["a.txt", "b.txt"], &one)),
		Err("git listed 2 staged files but its diff has 1; nothing was sent".to_string())
	);
}

#[test]
fn name_status_fields_become_labels_with_renames_as_old_and_new() {
	let out = b"M\0dir b/file.txt\0R087\0old.txt\0new.txt\0A\0\xc3\xa9.txt\0";
	assert_eq!(
		staged_labels(out),
		vec!["dir b/file.txt", "old.txt → new.txt", "é.txt"]
	);
	assert!(staged_labels(b"").is_empty());
}

/// Real git output: a path with a space before `b/`, one git would quote (`core.quotePath`), and
/// a rename. Each is named exactly, and the left-out note names the big file, not a fragment.
#[test]
fn real_staged_paths_are_named_as_git_names_them() {
	let repo = TempRepo::new("suggest-paths");
	repo.write("old.txt", "one\ntwo\nthree\nfour\nfive\n");
	repo.write("dir b/file.txt", "before\n");
	repo.commit_all("base");
	repo.git(&["mv", "old.txt", "new.txt"]);
	repo.write("dir b/file.txt", "after\n");
	repo.write("é.txt", "accented\n");
	repo.write("zz big.json", &"x".repeat(CAP + 10));
	repo.git(&["add", "-A"]);
	let files = staged_files(&repo.path).expect("staged files");
	assert_eq!(
		files,
		vec![
			"dir b/file.txt",
			"old.txt → new.txt",
			"zz big.json",
			"é.txt"
		]
	);
	let diff = read_diff_bytes(&repo.path, &DiffSource::Staged, &[]).expect("staged diff");
	let built = request(&Staged { files, diff }).expect("the list and the diff line up");
	for label in [
		"dir b/file.txt",
		"old.txt → new.txt",
		"zz big.json",
		"é.txt",
	] {
		assert!(
			built.text.contains(&format!("- {label}\n")),
			"{label} is listed"
		);
	}
	assert_eq!(built.left_out, vec!["zz big.json", "é.txt"]);
	assert!(
		built.text.contains("+after"),
		"the first file's diff is sent"
	);
}
