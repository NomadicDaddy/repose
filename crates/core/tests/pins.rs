//! repository-pinning clauses 1, 2 and 5 against real files: a pin save rewrites only
//! `pinned_repositories`, a reload reads it back, and a write that fails leaves the old file.
#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};

use repose_core::config::{self, ConfigError};
use repose_core::pins;

const WRITTEN: &str = r#"# My roots, chosen by hand.
roots = ['D:\applications', "E:\\work"]   # two drives
exclude = ["node_modules"]
depth = 2

[commands]
# The editor I use.
editor = "code"
terminal = "wt"
"#;

fn scratch(name: &str) -> PathBuf {
	let dir = std::env::temp_dir()
		.join("repose-pins-test")
		.join(format!("{name}-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("scratch dir");
	dir.join("config.toml")
}

#[test]
fn a_pin_save_changes_only_its_own_key_and_reloads() {
	let path = scratch("save");
	fs::write(&path, WRITTEN).expect("write config");
	assert!(
		config::load_or_create(&path)
			.expect("load")
			.pinned_repositories
			.is_empty()
	);

	let pins = pins::pinned(&[], Path::new("D:\\applications\\repose"));
	let pins = pins::pinned(&pins, Path::new("d:/APPLICATIONS/repose"));
	pins::save(&path, &pins).expect("save");
	let text = fs::read_to_string(&path).expect("read back");
	let added: Vec<&str> = text
		.lines()
		.filter(|l| l.starts_with("pinned_repositories"))
		.collect();
	assert_eq!(added.len(), 1, "one new key line:\n{text}");
	let rest: String = text
		.lines()
		.filter(|l| !l.starts_with("pinned_repositories"))
		.map(|l| format!("{l}\n"))
		.collect();
	assert_eq!(
		rest, WRITTEN,
		"every other line, comment and key is unchanged"
	);
	let loaded = config::load_or_create(&path).expect("reload");
	assert_eq!(
		loaded.pinned_repositories,
		vec![PathBuf::from("D:\\applications\\repose")]
	);
	assert_eq!(loaded.depth, 2);

	pins::save(
		&path,
		&pins::unpinned(
			&loaded.pinned_repositories,
			Path::new("D:/applications/repose"),
		),
	)
	.expect("unpin");
	let loaded = config::load_or_create(&path).expect("reload after unpin");
	assert!(loaded.pinned_repositories.is_empty());
	assert!(
		fs::read_to_string(&path)
			.expect("text")
			.contains("# The editor I use.")
	);
	assert!(
		!path.with_extension("toml.saving").exists(),
		"no staging file is left"
	);
}

#[test]
fn a_pin_for_a_missing_repository_stays_saved() {
	let path = scratch("missing");
	fs::write(&path, "roots = []\n").expect("write config");
	let gone = PathBuf::from("Q:\\no\\such\\root\\repo");
	pins::save(&path, std::slice::from_ref(&gone)).expect("save");
	let loaded = config::load_or_create(&path).expect("reload");
	assert_eq!(
		loaded.pinned_repositories,
		vec![gone],
		"kept for when its root returns"
	);
}

/// A config file Windows won't let the rename replace (read-only) fails the save with the file's
/// name, and leaves the old text and no staging file behind.
#[cfg(windows)]
#[test]
fn a_failed_pin_save_leaves_the_file_as_it_was() {
	let path = scratch("readonly");
	fs::write(&path, WRITTEN).expect("write config");
	let original = fs::metadata(&path).expect("meta").permissions();
	let mut locked = original.clone();
	locked.set_readonly(true);
	fs::set_permissions(&path, locked).expect("read-only");
	let result = pins::save(&path, &[PathBuf::from("D:\\applications\\repose")]);
	fs::set_permissions(&path, original).expect("writable again");
	match &result {
		Err(ConfigError::Write(p, _)) => assert_eq!(p, &path),
		other => panic!("expected a Write error naming the file, got {other:?}"),
	}
	let message = result.err().map(|e| e.to_string()).unwrap_or_default();
	assert!(message.starts_with("could not write "), "{message}");
	assert_eq!(fs::read_to_string(&path).expect("text"), WRITTEN);
	assert!(!path.with_extension("toml.saving").exists());
}
