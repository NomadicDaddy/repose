//! The tree reader against real git: nested folders, a name with a space, content read back byte
//! for byte, an older commit's tree, an unborn repository, and the ls-tree parser on a submodule
//! record.
#![allow(clippy::expect_used)]

mod support;

use repose_core::git::parse::parse_ls_tree;
use repose_core::git::tree::{is_binary, read_file, read_tree};
use support::TempRepo;

#[test]
fn lists_committed_paths_with_sizes_and_reads_content() {
	let repo = TempRepo::new("tree");
	assert!(
		read_tree(&repo.path, "HEAD").is_err(),
		"an unborn HEAD is an error, not an empty tree"
	);
	repo.write("README.md", "# hi\n");
	repo.write("src/lib.rs", "pub fn f() {}\n");
	repo.write("docs/with space.txt", "spaced\n");
	repo.commit_all("first");
	let first = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
	let edited = "pub fn f() -> u8 {\n\t1\n}\n";
	repo.write("src/lib.rs", edited);
	repo.write("src/new.rs", "x\n");
	repo.commit_all("second");

	let head = read_tree(&repo.path, "HEAD").expect("tree at HEAD");
	let paths: Vec<&str> = head.iter().map(|e| e.path.as_str()).collect();
	assert_eq!(
		paths,
		[
			"README.md",
			"docs/with space.txt",
			"src/lib.rs",
			"src/new.rs"
		],
		"git's own order, recursive, no tree rows"
	);
	let lib = head.iter().find(|e| e.path == "src/lib.rs").expect("lib");
	assert_eq!(
		lib.size,
		edited.len() as u64,
		"the size comes with the listing"
	);
	assert_eq!((lib.file_name(), lib.folder()), ("lib.rs", "src"));
	assert!(head.iter().all(|e| !e.submodule));
	assert_eq!(
		read_file(&repo.path, "HEAD", "docs/with space.txt").expect("spaced file"),
		b"spaced\n"
	);

	let old = read_tree(&repo.path, &first).expect("tree at the first commit");
	assert_eq!(old.len(), 3, "the first commit had no src/new.rs");
	assert_eq!(
		read_file(&repo.path, &first, "src/lib.rs").expect("old content"),
		b"pub fn f() {}\n",
		"a commit's copy, not the working tree's"
	);
	assert!(read_file(&repo.path, "HEAD", "missing.txt").is_err());
}

#[test]
fn submodule_records_and_binary_rule() {
	let bytes = b"160000 commit 1234567890abcdef1234567890abcdef12345678       -\tvendor/lib\x00100644 blob 89abcdef89abcdef89abcdef89abcdef89abcdef      12\ta.txt\0";
	let entries = parse_ls_tree(bytes);
	assert_eq!(entries.len(), 2);
	assert!(entries[0].submodule);
	assert_eq!(
		(entries[0].path.as_str(), entries[0].size),
		("vendor/lib", 0)
	);
	assert!(!entries[1].submodule);
	assert_eq!((entries[1].path.as_str(), entries[1].size), ("a.txt", 12));

	assert!(is_binary(b"\x89PNG\r\n\x1a\n\0\0"));
	assert!(!is_binary(b"plain text\n"));
	let mut late = vec![b'a'; 8000];
	late.push(0);
	assert!(
		!is_binary(&late),
		"a NUL after the first 8000 bytes does not count"
	);
}
