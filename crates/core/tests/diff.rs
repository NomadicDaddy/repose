//! Diff clause 1 against real git: the three sources, a rename read as a rename in the staged and
//! the committed diff, diff.noprefix not changing the parse, a binary file, and blob sizes.
#![allow(clippy::expect_used)]

mod support;

use repose_core::git::diff::{blob_size, read_diff};
use repose_core::model::{DiffSource, LineKind};
use support::TempRepo;

#[test]
fn rename_is_a_rename_in_staged_and_committed_diffs_whatever_noprefix_says() {
	let repo = TempRepo::new("diff-rename");
	repo.write(
		"old.txt",
		"line one\nline two\nline three\nline four\nline five\n",
	);
	repo.commit_all("base");
	repo.git(&["mv", "old.txt", "new.txt"]);
	repo.write(
		"new.txt",
		"line one\nline TWO\nline three\nline four\nline five\n",
	);
	repo.git(&["add", "new.txt"]);
	let both = ["old.txt".to_string(), "new.txt".to_string()];

	let staged = read_diff(&repo.path, &DiffSource::Staged, &both).expect("staged");
	assert_eq!(staged.len(), 1, "{staged:?}");
	assert!(staged[0].is_rename(), "{:?}", staged[0]);
	assert_eq!(staged[0].old_path.as_deref(), Some("old.txt"));
	assert_eq!(staged[0].new_path.as_deref(), Some("new.txt"));
	assert_eq!((staged[0].added(), staged[0].removed()), (1, 1));
	let changed = staged[0].hunks[0]
		.lines
		.iter()
		.find(|l| l.kind == LineKind::Added)
		.expect("added line");
	assert_eq!(changed.text, "line TWO");
	assert_eq!(changed.new_no, Some(2));

	repo.git(&["commit", "-q", "-m", "rename and edit"]);
	let head = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
	repo.git(&["config", "diff.noprefix", "true"]);
	let committed =
		read_diff(&repo.path, &DiffSource::Commit(head.clone()), &both).expect("committed");
	assert_eq!(
		committed, staged,
		"noprefix=true changes nothing the parser sees"
	);

	repo.git(&["config", "diff.mnemonicPrefix", "true"]);
	let again = read_diff(&repo.path, &DiffSource::Commit(head), &both).expect("committed again");
	assert_eq!(again, staged);
}

#[test]
fn working_tree_binary_and_sizes() {
	let repo = TempRepo::new("diff-working");
	repo.write("a.txt", "a\nb\nc\n");
	std::fs::write(repo.path.join("p.bin"), [0u8, 159, 146, 150, 0, 7]).expect("binary");
	repo.commit_all("base");
	repo.write("a.txt", "a\nB\nc\n");
	std::fs::write(repo.path.join("p.bin"), [0u8, 1, 2, 3, 0, 9]).expect("binary edit");

	let working =
		read_diff(&repo.path, &DiffSource::Working, &["a.txt".to_string()]).expect("working");
	assert_eq!(working.len(), 1);
	assert!(!working[0].is_rename());
	assert_eq!(working[0].hunks[0].lines.len(), 4);
	assert_eq!(working[0].hunks[0].old_start, 1);

	let binary =
		read_diff(&repo.path, &DiffSource::Working, &["p.bin".to_string()]).expect("binary diff");
	assert_eq!(binary.len(), 1);
	assert!(binary[0].binary, "{:?}", binary[0]);
	assert!(binary[0].hunks.is_empty());

	assert_eq!(blob_size(&repo.path, ":0", "a.txt").expect("index size"), 6);
	let head = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
	assert_eq!(
		blob_size(&repo.path, &head, "p.bin").expect("commit size"),
		6
	);
	assert!(
		read_diff(&repo.path, &DiffSource::Staged, &["a.txt".to_string()])
			.expect("nothing staged")
			.is_empty()
	);
}
