//! The log and name-status parsers against real git: multi-line bodies, a quoted subject, a
//! rename and a merge commit's first-parent files.
#![allow(clippy::expect_used)]

mod support;

use repose_core::git::log::{read_commit_files, read_log};
use repose_core::model::ChangeKind;
use support::TempRepo;

#[test]
fn pages_bodies_quotes_and_order() {
	let repo = TempRepo::new("log");
	repo.write("a.txt", "a\n");
	repo.git(&["add", "a.txt"]);
	repo.git(&[
		"commit",
		"-q",
		"-m",
		"first subject",
		"-m",
		"body line one\nbody line two",
	]);
	repo.write("b.txt", "b\n");
	repo.git(&["add", "b.txt"]);
	repo.git(&["commit", "-q", "-m", "second \"quoted\" subject"]);
	let log = read_log(&repo.path, 0).expect("log");
	assert_eq!(log.len(), 2);
	assert_eq!(log[0].subject, "second \"quoted\" subject");
	assert_eq!(log[0].body, "");
	assert_eq!(log[1].subject, "first subject");
	assert_eq!(log[1].body, "body line one\nbody line two");
	assert_eq!(log[0].author, "Test");
	assert_eq!(log[0].short.len(), 7);
	assert!(log[0].time > 1_700_000_000, "time parsed: {}", log[0].time);
	assert!(read_log(&repo.path, 2).expect("past the end").is_empty());
}

#[test]
fn commit_files_with_rename_and_merge() {
	let repo = TempRepo::new("files");
	repo.write("old.txt", "same content for the rename detector\n");
	repo.write("keep.txt", "k\n");
	repo.commit_all("base");
	repo.git(&["mv", "old.txt", "new.txt"]);
	repo.write("keep.txt", "k2\n");
	repo.commit_all("rename and edit");
	let head = repo.git(&["rev-parse", "HEAD"]);
	let files = read_commit_files(&repo.path, head.trim()).expect("files");
	let renamed = files
		.iter()
		.find(|f| f.kind == ChangeKind::Renamed)
		.expect("rename");
	assert_eq!(renamed.path, "new.txt");
	assert_eq!(renamed.from.as_deref(), Some("old.txt"));
	assert!(
		files
			.iter()
			.any(|f| f.path == "keep.txt" && f.kind == ChangeKind::Modified)
	);

	repo.git(&["checkout", "-q", "-b", "side"]);
	repo.write("side.txt", "s\n");
	repo.commit_all("side work");
	repo.git(&["checkout", "-q", "main"]);
	repo.write("main.txt", "m\n");
	repo.commit_all("main work");
	repo.git(&["merge", "-q", "--no-ff", "-m", "merge side", "side"]);
	let merge = repo.git(&["rev-parse", "HEAD"]);
	let merged = read_commit_files(&repo.path, merge.trim()).expect("merge files");
	assert!(
		merged
			.iter()
			.any(|f| f.path == "side.txt" && f.kind == ChangeKind::Added),
		"merge lists its first-parent changes: {merged:?}"
	);
	assert!(
		!merged.iter().any(|f| f.path == "main.txt"),
		"main.txt was already on main"
	);
}
