//! The log and name-status parsers against real git: multi-line bodies, a quoted subject, a
//! rename and a merge commit's first-parent files, and a second page of history past the first.
#![allow(clippy::expect_used)]

mod support;

use std::collections::HashSet;

use repose_core::git::log::{PAGE, read_commit_files, read_log, read_outgoing};
use repose_core::model::ChangeKind;
use support::TempRepo;

/// Morticia M-2 on 51bc93d: after a merge, the top `ahead` rows of the date-ordered log are not
/// the unpushed commits. The set from `<upstream>..HEAD` is.
#[test]
fn outgoing_is_the_set_not_the_top_of_the_log() {
	let repo = TempRepo::new("outgoing");
	repo.write("base.txt", "base\n");
	repo.commit_all("base");
	repo.git(&["branch", "up"]);
	repo.write("mine.txt", "mine\n");
	repo.commit_all("MINE local work");
	repo.git(&["checkout", "-q", "up"]);
	repo.write("theirs.txt", "theirs\n");
	repo.commit_all("THEIRS already on upstream");
	repo.git(&["checkout", "-q", "main"]);
	repo.git(&["merge", "-q", "--no-ff", "-m", "MERGE local merge", "up"]);
	repo.git(&["branch", "-u", "up"]);
	let log = read_log(&repo.path, 0).expect("log");
	let subject = |needle: &str| {
		log.iter()
			.find(|c| c.subject.starts_with(needle))
			.map(|c| c.hash.clone())
			.expect(needle)
	};
	let outgoing = read_outgoing(&repo.path, "up").expect("outgoing");
	let expected: HashSet<String> = [subject("MINE"), subject("MERGE")].into_iter().collect();
	assert_eq!(
		outgoing, expected,
		"THEIRS is on the upstream and stays out"
	);
	assert_eq!(log[0].subject, "MERGE local merge", "newest first");
}

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

/// History's Load more asks for the next page by skipping the commits it already holds. With one
/// commit more than a page, the first page is the newest PAGE, the second holds only the oldest,
/// and a skip past the end is empty; the shorter page is what marks the list exhausted.
#[test]
fn a_second_page_holds_what_the_first_page_left() {
	assert_eq!(PAGE, 100, "the page size History and this test share");
	let repo = TempRepo::new("pages");
	for n in 1..=PAGE + 1 {
		repo.git(&["commit", "-q", "--allow-empty", "-m", &format!("c{n}")]);
	}
	let first = read_log(&repo.path, 0).expect("first page");
	let subjects: Vec<&str> = first.iter().map(|c| c.subject.as_str()).collect();
	let newest: Vec<String> = (2..=PAGE + 1).rev().map(|n| format!("c{n}")).collect();
	assert_eq!(subjects, newest, "the newest PAGE commits, newest first");
	let second = read_log(&repo.path, PAGE).expect("second page");
	let rest: Vec<&str> = second.iter().map(|c| c.subject.as_str()).collect();
	assert_eq!(rest, ["c1"], "only the oldest commit is left");
	assert!(second.len() < PAGE, "a short page ends the list");
	assert!(
		read_log(&repo.path, 2 * PAGE)
			.expect("past the end")
			.is_empty()
	);
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
