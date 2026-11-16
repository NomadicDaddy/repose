//! conflict-stage-waits-for-markers criterion 4: in a temporary repository with a real merge
//! conflict, Merge Changes' Stage is disabled while x.txt holds conflict markers (a click queues
//! no git add and the file stays unmerged), and enabled once the markers are removed, when the
//! same click queues the Stage write exactly as before.

use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

use eframe::egui;
use repose_core::config::Config;
use repose_core::git::read_status_into;
use repose_core::model::MarkerCheck;

use super::ui_harness::{click, frame, real_repo, repo_at, text_at, texts};
use super::{AppState, View, WriteKind};

fn git(dir: &Path, args: &[&str]) -> String {
	let out = Command::new("git")
		.args([
			"-c",
			"user.name=repose test",
			"-c",
			"user.email=repose@example.invalid",
		])
		.args(args)
		.current_dir(dir)
		.output()
		.unwrap_or_else(|e| panic!("git {}: {e}", args.join(" ")));
	String::from_utf8_lossy(&out.stdout).to_string()
}

/// x.txt edited on two branches and merged, so git leaves it unmerged with markers in it.
fn conflicted(name: &str) -> std::path::PathBuf {
	let dir = real_repo(name);
	git(&dir, &["checkout", "-q", "-b", "other"]);
	std::fs::write(dir.join("x.txt"), "theirs\n").unwrap_or_else(|e| panic!("{e}"));
	git(&dir, &["commit", "-q", "-am", "theirs"]);
	git(&dir, &["checkout", "-q", "-"]);
	std::fs::write(dir.join("x.txt"), "ours\n").unwrap_or_else(|e| panic!("{e}"));
	git(&dir, &["commit", "-q", "-am", "ours"]);
	git(&dir, &["merge", "-q", "other"]);
	assert!(git(&dir, &["status", "--porcelain"]).contains("UU x.txt"));
	dir
}

fn select_conflict(state: &mut AppState, dir: &Path) {
	let mut row = repo_at(dir.to_path_buf(), SystemTime::now());
	read_status_into(&mut row);
	state.selected = Some(row.path.clone());
	state.merge_discovered(vec![row]);
	state.view = View::Changes;
	state.selected_file = Some("x.txt".into());
}

fn click_stage(ctx: &egui::Context, state: &mut AppState) {
	let shapes = frame(ctx, state, Vec::new());
	let Some(stage) = text_at(&shapes, "Stage") else {
		panic!("no Stage on the conflict row: {:?}", texts(&shapes));
	};
	frame(ctx, state, click(stage));
}

#[test]
fn stage_waits_until_the_conflict_markers_are_gone() {
	let dir = conflicted("conflict-stage-ui");
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	select_conflict(&mut state, &dir);
	let changes = state.repos[0].changes.clone().unwrap_or_default();
	assert_eq!(changes.marker_check("x.txt"), MarkerCheck::Present);
	frame(&ctx, &mut state, Vec::new());
	assert!(
		texts(&frame(&ctx, &mut state, Vec::new()))
			.iter()
			.any(|t| t == "Merge Changes")
	);

	click_stage(&ctx, &mut state);
	assert!(
		state.writes.running.is_empty() && state.writes.waiting.is_empty(),
		"a click on Stage with markers queues no write"
	);
	assert!(
		git(&dir, &["status", "--porcelain"]).contains("UU x.txt"),
		"still unmerged"
	);

	std::fs::write(dir.join("x.txt"), "resolved\n").unwrap_or_else(|e| panic!("{e}"));
	// A click on the disabled Stage falls through to its row and opens the diff, so the enabled
	// case starts from a fresh window on the next status read.
	let ctx = egui::Context::default();
	let mut state = AppState::new(Config::default());
	select_conflict(&mut state, &dir);
	let changes = state.repos[0].changes.clone().unwrap_or_default();
	assert_eq!(changes.marker_check("x.txt"), MarkerCheck::Clear);
	frame(&ctx, &mut state, Vec::new());
	click_stage(&ctx, &mut state);
	let queued = state.writes.running.get(&dir).map(|r| r.kind.clone());
	assert!(
		matches!(&queued, Some(WriteKind::Stage(paths)) if paths == &["x.txt"]),
		"Stage queues git add for x.txt once the markers are gone: {queued:?}"
	);
	let _ = std::fs::remove_dir_all(&dir);
}
