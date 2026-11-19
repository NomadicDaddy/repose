//! ai-commit-message-suggestions: what a request carries. Every staged file is listed by git's own
//! name for it, read from real git output (a space before `b/`, a name git quotes, a rename),
//! whole-file diffs follow up to the 64 KiB cap, and a diff that does not line up with the list is
//! refused rather than guessed at (Roger 5Q-2 on d3da4dd). The list and the diff come from one
//! git read, so they always describe the same index (Roger 5Q-3 on 987f892).
#![allow(clippy::expect_used)]

mod support;

use repose_core::git::diff::{read_diff_bytes, split_raw, staged_for_suggestion};
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

/// A file whose own name holds the rename arrow, under `core.quotePath=false` so git leaves its
/// header unquoted, is listed and sent like any other (Roger on 2a0b437): nothing reads paths back
/// out of a label.
#[test]
fn a_filename_with_the_rename_arrow_is_sent_like_any_other() {
	let repo = TempRepo::new("suggest-arrow");
	repo.git(&["config", "core.quotePath", "false"]);
	repo.write("base.txt", "base\n");
	repo.commit_all("base");
	repo.write("notes → draft.txt", "draft\n");
	repo.git(&["add", "-A"]);
	let (files, diff) = staged_for_suggestion(&repo.path).expect("staged read");
	assert_eq!(files, vec!["notes → draft.txt"]);
	let header = String::from_utf8_lossy(&diff)
		.lines()
		.next()
		.unwrap_or_default()
		.to_string();
	assert_eq!(header, "diff --git a/notes → draft.txt b/notes → draft.txt");
	let built = request(&Staged { files, diff }).expect("the request is built");
	assert!(built.text.contains("- notes → draft.txt\n"));
	assert!(built.text.contains("+draft"));
}

#[test]
fn raw_entries_become_labels_with_renames_as_old_and_new() {
	let mut out = b":100644 100644 aaa bbb M\0dir b/file.txt\0".to_vec();
	out.extend(b":100644 100644 ccc ccc R100\0old.txt\0new.txt\0");
	out.extend(b":000000 100644 000 ddd A\0\xc3\xa9.txt\0\0diff --git a/x b/x\n");
	let (labels, patch) = split_raw(&out);
	assert_eq!(labels, vec!["dir b/file.txt", "old.txt → new.txt", "é.txt"]);
	assert_eq!(patch, b"diff --git a/x b/x\n");
	assert_eq!(split_raw(b""), (Vec::new(), Vec::new()));
}

/// Real git output: a path with a space before `b/`, one git would quote (`core.quotePath`), and
/// a rename, all from one read. Each is named exactly, and the left-out note names the big file.
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
	let (files, diff) = staged_for_suggestion(&repo.path).expect("staged read");
	let labels = [
		"dir b/file.txt",
		"old.txt → new.txt",
		"zz big.json",
		"é.txt",
	];
	assert_eq!(files, labels);
	let built = request(&Staged { files, diff }).expect("the list and the diff line up");
	for label in labels {
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

/// A staged submodule under a user's `diff.submodule=log` sends only its two commit ids, never
/// the submodule's commit subjects, and its section still lines up with the list (Morticia M-1 on
/// d3da4dd). The plain staged read shows the subjects, so the test can see what it guards.
#[test]
fn a_staged_submodule_sends_its_commit_ids_and_never_its_subjects() {
	let repo = TempRepo::new("suggest-submodule");
	let sub = repo.path.join("s");
	std::fs::create_dir_all(&sub).expect("submodule dir");
	support::git_in(&sub, &["init", "-q", "-b", "main"]);
	support::set_identity(&sub);
	std::fs::write(sub.join("a.txt"), "one\n").expect("write");
	support::git_in(&sub, &["add", "-A"]);
	support::git_in(&sub, &["commit", "-q", "-m", "first"]);
	let first = support::git_in(&sub, &["rev-parse", "HEAD"])
		.trim()
		.to_string();
	std::fs::write(sub.join("a.txt"), "two\n").expect("write");
	support::git_in(&sub, &["commit", "-q", "-am", "SECRET-SUBJECT-TWO"]);
	let second = support::git_in(&sub, &["rev-parse", "HEAD"])
		.trim()
		.to_string();
	let entry = |id: &str| format!("160000,{id},s");
	repo.git(&["update-index", "--add", "--cacheinfo", &entry(&first)]);
	repo.git(&["commit", "-q", "-m", "base"]);
	repo.git(&["update-index", "--cacheinfo", &entry(&second)]);
	repo.git(&["config", "diff.submodule", "log"]);
	let plain = read_diff_bytes(&repo.path, &DiffSource::Staged, &[]).expect("plain read");
	assert!(
		String::from_utf8_lossy(&plain).contains("SECRET-SUBJECT-TWO"),
		"the config does add subjects to a plain read"
	);
	let (files, diff) = staged_for_suggestion(&repo.path).expect("suggestion read");
	let text = String::from_utf8_lossy(&diff).to_string();
	assert!(!text.contains("SECRET-SUBJECT-TWO"), "{text}");
	assert!(
		text.contains(&format!("+Subproject commit {second}")),
		"{text}"
	);
	assert_eq!(files, vec!["s"]);
	let built = request(&Staged { files, diff }).expect("one section for one file");
	assert!(!built.text.contains("SECRET-SUBJECT-TWO"));
}
