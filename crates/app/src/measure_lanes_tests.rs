//! The `--measure-lanes` probe's report and its real-input stop, without a window.

use eframe::egui::{self, Event, RawInput, pos2};
use repose_core::config::Config;

use super::{LanesProbe, percentile, report};
use crate::state::AppState;

#[test]
fn percentiles_take_the_nearest_rank() {
	let v: Vec<f64> = (1..=100).map(f64::from).collect();
	assert_eq!(percentile(&v, 0.5), 50.0);
	assert_eq!(percentile(&v, 0.95), 95.0);
	assert_eq!(percentile(&[], 0.5), 0.0);
}

/// The report's shape, and the pass rule each way it can fail: a run at the refresh rate
/// passes; one whose p95 frame takes two intervals fails with its misses counted; and one
/// with too few moving frames fails however fast they were.
#[test]
fn the_report_applies_the_pass_rule_it_states() {
	let page = [("commits", "100".to_string())];
	let idle = vec![16.67; 60];
	let good = report(&page, &idle, 100, &vec![16.7; 100], &vec![3.0; 100]);
	assert!(
		good.starts_with(
			"commits\t100\nrefresh_ms\t16.67\nwheel_frames\t100\nmoving_frames\t100\n"
		),
		"{good}"
	);
	assert!(
		good.contains("missed\t0\n") && good.ends_with("pass\tyes\n"),
		"{good}"
	);
	let mut slow = vec![16.7; 90];
	slow.extend(vec![33.4; 10]);
	let bad = report(&page, &idle, 100, &slow, &vec![3.0; 100]);
	assert!(
		bad.contains("missed\t10\n") && bad.ends_with("pass\tno\n"),
		"{bad}"
	);
	let few = report(&page, &idle, 100, &vec![16.7; 69], &vec![3.0; 69]);
	assert!(
		few.contains("moving_frames\t69\n") && few.ends_with("pass\tno\n"),
		"{few}"
	);
}

/// #39830, #39831: real input on the desk ends the run with a failed report. The pointer
/// resting where it was first seen is not input; moving from there is, and so is a key.
#[test]
fn real_input_during_the_run_ends_it_with_a_failed_report() {
	let ctx = egui::Context::default();
	let state = AppState::new(Config::default());
	let frame = |probe: &mut LanesProbe, events: Vec<Event>| {
		let mut raw = RawInput {
			events,
			..RawInput::default()
		};
		probe.inject(&mut raw);
		probe.begin();
		probe.end(&state, &ctx)
	};
	let mut probe = LanesProbe::new("unused.txt".into());
	let resting = vec![Event::PointerMoved(pos2(10.0, 10.0))];
	assert_eq!(
		frame(&mut probe, resting.clone()),
		None,
		"first sight of the pointer"
	);
	assert_eq!(frame(&mut probe, resting), None, "still resting there");
	let moved = frame(&mut probe, vec![Event::PointerMoved(pos2(11.0, 10.0))]);
	assert_eq!(
		moved.as_deref(),
		Some("error\treal input during the run: the pointer moving\n")
	);
	let mut probe = LanesProbe::new("unused.txt".into());
	let key = Event::Text("a".into());
	let typed = frame(&mut probe, vec![key]);
	assert_eq!(
		typed.as_deref(),
		Some("error\treal input during the run: a key\n")
	);
}
