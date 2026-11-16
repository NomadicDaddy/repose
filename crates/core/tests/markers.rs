//! conflict-stage-waits-for-markers criteria 2 and 4: the marker check on a conflicted working
//! copy, and that a status read records it for each conflicted path without staging anything.
#![allow(clippy::expect_used)]

mod support;

use repose_core::git::markers::check_markers;
use repose_core::git::read_status_into;
use repose_core::model::{MarkerCheck, RepoKind, RepoSummary};
use support::TempRepo;

#[test]
fn each_marker_line_counts_and_lookalikes_do_not() {
	let repo = TempRepo::new("markers-lines");
	for (text, expected) in [
		("a\n<<<<<<< HEAD\nb\n", MarkerCheck::Present),
		(
			"<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> other\n",
			MarkerCheck::Present,
		),
		(
			"<<<<<<< HEAD\r\nours\r\n=======\r\ntheirs\r\n>>>>>>> other\r\n",
			MarkerCheck::Present,
		),
		("a\n>>>>>>> other\n", MarkerCheck::Present),
		// A lone seven-'=' line is a Markdown or RST heading underline, not a marker, so a
		// resolved .md or .rst can be staged (Morticia Low 1 on 5904afc).
		("Title\n=======\n\nText.\n", MarkerCheck::Clear),
		("a\r\n=======\r\nb\r\n", MarkerCheck::Clear),
		(
			"a\n======== heading underline\n<<<<<<<no space\n",
			MarkerCheck::Clear,
		),
		("resolved\n", MarkerCheck::Clear),
	] {
		repo.write("f.txt", text);
		assert_eq!(
			check_markers(&repo.path.join("f.txt")),
			expected,
			"{text:?}"
		);
	}
}

#[test]
fn binary_and_missing_files_cannot_be_checked() {
	let repo = TempRepo::new("markers-unchecked");
	std::fs::write(repo.path.join("b.bin"), [0x89, b'P', 0, b'<', b'\n']).expect("write");
	assert_eq!(
		check_markers(&repo.path.join("b.bin")),
		MarkerCheck::Unchecked
	);
	assert_eq!(
		check_markers(&repo.path.join("gone.txt")),
		MarkerCheck::Unchecked
	);
}

fn conflicted(label: &str, theirs: &str, ours: &str) -> TempRepo {
	let repo = TempRepo::new(label);
	repo.write("x.txt", "base\n");
	repo.commit_all("base");
	repo.git(&["checkout", "-q", "-b", "other"]);
	repo.write("x.txt", theirs);
	repo.commit_all("theirs");
	repo.git(&["checkout", "-q", "main"]);
	repo.write("x.txt", ours);
	repo.commit_all("ours");
	let _ = std::process::Command::new("git")
		.args(["merge", "-q", "other"])
		.current_dir(&repo.path)
		.env("GIT_CONFIG_GLOBAL", "NUL")
		.output();
	repo
}

fn summary(repo: &TempRepo) -> RepoSummary {
	let mut s = RepoSummary::new(repo.path.clone(), RepoKind::Repository);
	read_status_into(&mut s);
	s
}

#[test]
fn a_status_read_records_markers_then_their_removal_and_stages_nothing() {
	let repo = conflicted("markers-status", "theirs\n", "ours\n");
	let s = summary(&repo);
	let changes = s.changes.expect("changes");
	assert_eq!(changes.conflicts.len(), 1);
	assert_eq!(changes.marker_check("x.txt"), MarkerCheck::Present);
	assert!(
		repo.git(&["status", "--porcelain"]).contains("UU x.txt"),
		"nothing staged"
	);

	repo.write("x.txt", "resolved\n");
	let changes = summary(&repo).changes.expect("changes");
	assert_eq!(changes.marker_check("x.txt"), MarkerCheck::Clear);
	assert!(
		repo.git(&["status", "--porcelain"]).contains("UU x.txt"),
		"still unmerged"
	);

	std::fs::remove_file(repo.path.join("x.txt")).expect("remove");
	let changes = summary(&repo).changes.expect("changes");
	assert_eq!(
		changes.marker_check("x.txt"),
		MarkerCheck::Unchecked,
		"missing file"
	);
}

#[test]
fn a_path_never_checked_reads_as_unchecked() {
	let changes = repose_core::model::Changes::default();
	assert_eq!(changes.marker_check("anything"), MarkerCheck::Unchecked);
}
