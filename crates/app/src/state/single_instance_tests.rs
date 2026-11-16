//! close-to-tray clause 5 (single_instance.rs): the hand-off's encoding, the lock per config
//! folder, and a hand-off applied as a start-up would apply it (Roger 5Q-1 and 5Q-5 on 0a3048d).

use std::fs;
use std::path::PathBuf;

use eframe::egui;

use super::{AppState, View};
use crate::single_instance::{Handoff, Instance, acquire, apply, decode, encode};

#[test]
fn a_handoff_round_trips_and_ignores_what_it_does_not_know() {
	let h = Handoff {
		select: Some(PathBuf::from("D:\\applications\\repose")),
		view: Some(View::History),
		file: Some(("src/main.rs".into(), true)),
		open: Some("README.md".into()),
		lanes: true,
	};
	assert_eq!(decode(&encode(&h)), h);
	let plain = Handoff {
		file: Some(("a.txt".into(), false)),
		..Handoff::default()
	};
	assert_eq!(decode(&encode(&plain)), plain);
	assert_eq!(
		decode("view=sideways\nnoise\nselect=\n"),
		Handoff::default()
	);
	assert_eq!(decode(""), Handoff::default());
}

/// A window listing one repository at `D:\fx\listed`, with no roots, so a scan finds nothing.
fn listed_state() -> AppState {
	use repose_core::config::Config;
	use repose_core::model::{RepoKind, RepoSummary};
	let mut state = AppState::new(Config {
		roots: Vec::new(),
		..Config::default()
	});
	let row = RepoSummary::new(PathBuf::from("D:\\fx\\listed"), RepoKind::Repository);
	state.merge_discovered(vec![row]);
	state
}

/// Roger 5Q-1 and 5Q-5 on 0a3048d: a hand-off acts as a start-up does.
#[test]
fn a_handoff_selects_now_or_waits_for_the_scan_and_keeps_every_flag() {
	let ctx = egui::Context::default();

	let mut state = listed_state();
	let listed = Handoff {
		select: Some(PathBuf::from("d:/FX/listed")),
		view: Some(View::Branches),
		lanes: true,
		..Handoff::default()
	};
	apply(&ctx, &mut state, listed);
	assert_eq!(state.selected, Some(PathBuf::from("D:\\fx\\listed")));
	assert!(
		state.requested_selection.is_none() && !state.scanning,
		"selected at once"
	);
	assert_eq!(state.view, View::Branches);
	assert!(state.lanes);

	let mut state = listed_state();
	let later = Handoff {
		select: Some(PathBuf::from("D:\\fx\\not-yet")),
		..Handoff::default()
	};
	apply(&ctx, &mut state, later);
	assert_eq!(
		state.requested_selection,
		Some(PathBuf::from("D:\\fx\\not-yet"))
	);
	assert!(
		state.scanning,
		"it waits for a scan's result instead of being dropped"
	);

	let mut state = listed_state();
	state.select(PathBuf::from("D:\\fx\\listed"));
	let files = Handoff {
		file: Some(("a.txt".into(), true)),
		open: Some("README.md".into()),
		..Handoff::default()
	};
	apply(&ctx, &mut state, files);
	assert_eq!(state.requested_diff, Some(("a.txt".into(), true)));
	assert_eq!(state.requested_file.as_deref(), Some("README.md"));
	assert_eq!(
		state.requested_selection,
		Some(PathBuf::from("D:\\fx\\listed"))
	);
	assert!(
		state.scanning,
		"--file and --open land through a scan, as at start-up"
	);
}

#[test]
fn a_second_process_with_the_same_config_is_secondary_and_another_config_is_not() {
	let root = std::env::temp_dir().join(format!("repose-single-{}", std::process::id()));
	let _ = fs::remove_dir_all(&root);
	let first = acquire(&root.join("a").join("config.toml"));
	assert!(matches!(first, Instance::Primary { .. }));
	let again = acquire(&root.join("a").join("config.toml"));
	assert!(
		matches!(again, Instance::Secondary { .. }),
		"the lock is held"
	);
	let other = acquire(&root.join("b").join("config.toml"));
	assert!(
		matches!(other, Instance::Primary { .. }),
		"another config folder"
	);
	drop((first, again, other));
	let after = acquire(&root.join("a").join("config.toml"));
	assert!(
		matches!(after, Instance::Primary { .. }),
		"released when the holder ends"
	);
	drop(after);
	let _ = fs::remove_dir_all(&root);
}
