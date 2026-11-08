//! The `--measure-lanes` probe's report and its real-input stop, without a window.

use eframe::egui::{self, Event, RawInput, pos2};
use repose_core::config::Config;

use super::{LanesProbe, Phase, SCROLL_FRAMES, percentile, report};
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

/// Roger on fd6f7c2 and c69a098: each interval belongs to the frame whose render it covers, so
/// every moving frame has one interval and one ui() time, the last wheel frame included, and a
/// frame whose view did not move contributes neither.
#[test]
fn every_moving_frame_has_one_interval_and_one_ui_time() {
	let ctx = egui::Context::default();
	let state = AppState::new(Config::default());
	let mut probe = LanesProbe::new("unused.txt".into());
	probe.phase = Phase::Scrolling;
	probe.idle = vec![16.0; 60];
	let mut report = None;
	let mut moving = 0;
	let mut offset = 0.0_f32;
	for frame in 0..=SCROLL_FRAMES {
		// Every third wheel frame leaves the view where it was; the observation frame does too.
		if frame < SCROLL_FRAMES && frame % 3 != 2 {
			offset += 38.0;
			moving += 1;
		}
		ctx.data_mut(|d| d.insert_temp(crate::screens::detail::offset_key(), offset));
		let mut raw = RawInput::default();
		probe.inject(&mut raw);
		assert_eq!(
			raw.events.len(),
			if frame < SCROLL_FRAMES { 2 } else { 0 },
			"frame {frame}: the observation frame adds no input"
		);
		probe.begin();
		report = probe.end(&state, &ctx);
		assert_eq!(report.is_some(), frame == SCROLL_FRAMES, "frame {frame}");
	}
	// The first wheel frame has no offset before it to compare with, so it is never counted.
	let counted = moving - 1;
	assert_eq!(probe.cpu.len(), counted, "one ui() time per moving frame");
	assert_eq!(
		probe.present.len(),
		counted,
		"and one interval each, the last included"
	);
	assert!(
		report.is_some_and(|r| r.contains(&format!("moving_frames\t{counted}\n"))),
		"the report counts them"
	);
}
