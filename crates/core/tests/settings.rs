//! settings-screen clause 3 against real files: a settings save rewrites only the keys that
//! changed, keeps comments, unknown keys and the pins, creates `[commands]` when the file has
//! none, refuses a bad command before writing, and reloads as saved.
#![allow(clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

use repose_core::config::{self, Config, ConfigError, Theme};
use repose_core::settings;

const WRITTEN: &str = r#"# My roots, chosen by hand.
roots = ['D:\applications', "E:\\work"]   # two drives
exclude = ["node_modules"]
depth = 2
pinned_repositories = ['D:\applications\repose']
future_key = "kept"   # a key this version does not know

[commands]
# The editor I use.
editor = "code"
terminal = "wt"  # Windows Terminal
"#;

fn scratch(name: &str, text: &str) -> PathBuf {
	let dir = std::env::temp_dir()
		.join("repose-settings-test")
		.join(format!("{name}-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	fs::create_dir_all(&dir).expect("scratch dir");
	let path = dir.join("config.toml");
	fs::write(&path, text).expect("write config");
	path
}

/// The file's lines that do not start with any of `keys`.
fn lines_without(text: &str, keys: &[&str]) -> Vec<String> {
	text.lines()
		.filter(|l| !keys.iter().any(|k| l.starts_with(k)))
		.map(str::to_string)
		.collect()
}

#[test]
fn a_save_rewrites_only_the_keys_that_changed() {
	let path = scratch("changed", WRITTEN);
	let before = config::load_or_create(&path).expect("load");
	let mut next = before.clone();
	next.roots.push(PathBuf::from("F:\\more"));
	next.depth = 4;
	next.theme = Theme::Light;
	next.commands.terminal = "pwsh".into();
	settings::save(&path, &before, &next).expect("save");

	let text = fs::read_to_string(&path).expect("read back");
	let touched = ["roots", "depth", "theme", "terminal"];
	assert_eq!(
		lines_without(&text, &touched),
		lines_without(WRITTEN, &touched),
		"every other line, comment and key is unchanged:\n{text}"
	);
	let loaded = config::load_or_create(&path).expect("reload");
	assert_eq!(loaded, next, "it reads back as saved");
	assert_eq!(
		loaded.pinned_repositories,
		[PathBuf::from("D:\\applications\\repose")],
		"the pins are untouched"
	);
	assert!(text.contains("future_key = \"kept\""), "{text}");
	// A changed key keeps its own line's comment and spacing (Roger S1, Morticia 5Q-4).
	let line = |key: &str| {
		text.lines()
			.find(|l| l.starts_with(key))
			.unwrap_or_default()
			.to_string()
	};
	assert_eq!(
		line("roots"),
		r"roots = ['D:\applications', 'E:\work', 'F:\more']   # two drives"
	);
	assert_eq!(line("terminal"), r#"terminal = "pwsh"  # Windows Terminal"#);
}

#[test]
fn an_unchanged_save_leaves_the_file_byte_for_byte() {
	let path = scratch("unchanged", WRITTEN);
	let before = config::load_or_create(&path).expect("load");
	settings::save(&path, &before, &before).expect("save");
	assert_eq!(fs::read_to_string(&path).expect("read back"), WRITTEN);
}

#[test]
fn a_file_without_commands_gets_a_commands_table() {
	let path = scratch("no-commands", "roots = []\n");
	let before = config::load_or_create(&path).expect("load");
	let mut next = before.clone();
	next.commands.editor = "notepad".into();
	settings::save(&path, &before, &next).expect("save");
	let text = fs::read_to_string(&path).expect("read back");
	assert!(text.contains("[commands]"), "{text}");
	assert_eq!(config::load_or_create(&path).expect("reload"), next);
}

#[test]
fn a_bad_command_is_refused_and_nothing_is_written() {
	let path = scratch("bad-command", WRITTEN);
	let before = config::load_or_create(&path).expect("load");
	let next = Config {
		depth: 9,
		commands: config::Commands {
			editor: "code --wait".into(),
			..before.commands.clone()
		},
		..before.clone()
	};
	let refused = settings::save(&path, &before, &next);
	assert!(
		matches!(refused, Err(ConfigError::BadCommand(ref c)) if c == "code --wait"),
		"{refused:?}"
	);
	assert_eq!(fs::read_to_string(&path).expect("read back"), WRITTEN);
}

#[test]
fn command_ok_takes_names_and_absolute_paths_only() {
	assert!(config::command_ok("code"));
	assert!(
		!config::command_ok("C:\\Program Files\\x.exe"),
		"a space is refused"
	);
	assert!(config::command_ok("C:\\Tools\\editor.exe"));
	assert!(!config::command_ok(""));
	assert!(
		!config::command_ok("tools\\editor.exe"),
		"relative paths are refused"
	);
	assert!(!config::command_ok("code | more"));
}
