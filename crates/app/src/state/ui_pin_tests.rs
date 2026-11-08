//! repository-pinning clauses 2 to 6 through the real list: Pin and Unpin from a row's menu (the
//! Menu key on the selected row), pinned rows first with a "pinned" marker and the selection
//! kept, a nested worktree pinning its parent, a worktree's saved pin lifting its parent, the
//! save to a --config file read back by a new session, and a failed save shown and undone.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use eframe::egui::{self, Key};
use repose_core::config::{self, Config};
use repose_core::model::RepoSummary;

use super::AppState;
use super::ui_harness::{click, frame, key, repo, text_at, texts};

fn three() -> Vec<RepoSummary> {
	let t0 = SystemTime::now();
	vec![repo("alpha", t0), repo("beta", t0), repo("gamma", t0)]
}

fn names(state: &AppState) -> Vec<&str> {
	state.repos.iter().map(|r| r.name.as_str()).collect()
}

/// Opens the selected row's menu with the Menu key and clicks `item`.
fn choose(ctx: &egui::Context, state: &mut AppState, item: &str) {
	frame(ctx, state, key(Key::Menu));
	let open = frame(ctx, state, Vec::new());
	let at = text_at(&open, item).unwrap_or_else(|| panic!("no {item}: {:?}", texts(&open)));
	frame(ctx, state, click(at));
}

fn select(state: &mut AppState, name: &str) -> PathBuf {
	let path = state
		.repos
		.iter()
		.flat_map(|r| std::iter::once(r).chain(&r.worktrees))
		.find(|r| r.name == name)
		.map(|r| r.path.clone())
		.unwrap_or_else(|| panic!("no row {name}"));
	state.selected = Some(path.clone());
	path
}

#[test]
fn pin_moves_the_row_first_with_its_marker_and_unpin_puts_it_back() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	state.merge_discovered(three());
	let gamma = select(&mut state, "gamma");
	frame(&ctx, &mut state, Vec::new());
	choose(&ctx, &mut state, "Pin");
	assert_eq!(names(&state), ["gamma", "alpha", "beta"]);
	assert_eq!(
		state.visible_paths()[0],
		gamma,
		"the keyboard order is the drawn order"
	);
	assert_eq!(
		state.selected.as_ref(),
		Some(&gamma),
		"the selection is kept"
	);
	let shown = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(shown.iter().any(|t| t == "pinned"), "{shown:?}");

	state.filter = "a".into();
	assert_eq!(
		state.visible_paths()[0],
		gamma,
		"filtering keeps pinned rows first"
	);
	state.filter.clear();

	choose(&ctx, &mut state, "Unpin");
	assert_eq!(
		names(&state),
		["alpha", "beta", "gamma"],
		"back in name order"
	);
	let shown = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(!shown.iter().any(|t| t == "pinned"), "{shown:?}");
}

#[test]
fn a_nested_worktree_pins_its_parent_and_a_saved_worktree_pin_lifts_it() {
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	let mut rows = three();
	rows[1].worktrees.push(repo("beta-wt", SystemTime::now()));
	let beta = rows[1].path.clone();
	let worktree = rows[1].worktrees[0].path.clone();
	state.merge_discovered(rows.clone());
	state.expanded.insert(beta.clone(), true);
	select(&mut state, "beta-wt");
	frame(&ctx, &mut state, Vec::new());
	choose(&ctx, &mut state, "Pin parent repository");
	assert_eq!(names(&state), ["beta", "alpha", "gamma"]);
	assert_eq!(
		state.config.pinned_repositories,
		std::slice::from_ref(&beta)
	);
	assert_eq!(state.pin_label(&worktree), Some("Unpin parent repository"));

	// A pin saved while the worktree stood alone (its parent outside the roots) lifts the parent
	// once both are discovered together, and Unpin on the parent clears it.
	let mut later = AppState::new(Config {
		pinned_repositories: vec![worktree.clone()],
		..Config::default()
	});
	later.merge_discovered(rows);
	assert_eq!(names(&later), ["beta", "alpha", "gamma"]);
	assert_eq!(later.pin_label(&beta), Some("Unpin"));
	assert_eq!(later.toggle_pin(&beta), Ok(()));
	assert!(later.config.pinned_repositories.is_empty());
	assert_eq!(names(&later), ["alpha", "beta", "gamma"]);
}

fn config_file(name: &str) -> PathBuf {
	let dir = std::env::temp_dir()
		.join("repose-ui-pins")
		.join(format!("{name}-{}", std::process::id()));
	let _ = fs::remove_dir_all(&dir);
	if let Err(e) = fs::create_dir_all(&dir) {
		panic!("scratch dir: {e}");
	}
	let path = dir.join("config.toml");
	if let Err(e) = fs::write(&path, "# mine\nroots = []\n") {
		panic!("config: {e}");
	}
	path
}

fn load(path: &Path) -> Config {
	config::load_or_create(path).unwrap_or_else(|e| panic!("load: {e}"))
}

#[test]
fn a_pin_is_saved_to_the_config_file_and_a_new_session_starts_with_it() {
	let path = config_file("restart");
	let gone = PathBuf::from("Q:\\no\\such\\root\\old");
	let mut state = AppState::new(Config {
		pinned_repositories: vec![gone.clone()],
		..load(&path)
	});
	state.config_path = Some(path.clone());
	state.merge_discovered(three());
	let gamma = state.repos[2].path.clone();
	assert_eq!(state.toggle_pin(&gamma), Ok(()));
	let saved = load(&path);
	assert_eq!(
		saved.pinned_repositories,
		[gone, gamma.clone()],
		"the missing one stays"
	);
	let text = fs::read_to_string(&path).unwrap_or_default();
	assert!(text.starts_with("# mine\nroots = []\n"), "{text}");

	let mut next = AppState::new(saved);
	next.merge_discovered(three());
	assert_eq!(
		names(&next),
		["gamma", "alpha", "beta"],
		"pinned across launches"
	);
}

#[cfg(windows)]
#[test]
fn a_failed_save_shows_its_error_and_changes_nothing() {
	let ctx = egui::Context::default();
	let path = config_file("readonly");
	let original = fs::metadata(&path).map(|m| m.permissions());
	let Ok(original) = original else {
		panic!("metadata");
	};
	let mut locked = original.clone();
	locked.set_readonly(true);
	if let Err(e) = fs::set_permissions(&path, locked) {
		panic!("read-only: {e}");
	}
	let mut state = AppState::new(load(&path));
	state.config_path = Some(path.clone());
	state.merge_discovered(three());
	select(&mut state, "gamma");
	frame(&ctx, &mut state, Vec::new());
	choose(&ctx, &mut state, "Pin");
	let _ = fs::set_permissions(&path, original);
	assert_eq!(
		names(&state),
		["alpha", "beta", "gamma"],
		"the order is unchanged"
	);
	assert!(state.config.pinned_repositories.is_empty());
	let shown = texts(&frame(&ctx, &mut state, Vec::new()));
	assert!(
		shown.iter().any(|t| t.starts_with("The pin was not saved")),
		"{shown:?}"
	);
}
