//! Activity's days are local from the first frame
//! (remediation-20261116-activity-days-before-the-first-scan), driven through the real frame and
//! read back from the AccessKit tree. While the scan runs and the offset is unknown, no cell names
//! a day; once the offset is in, still mid-scan, a commit made today local time sits under the
//! local today, a date the UTC clock has already left; and a failed offset read settles on UTC
//! when the scan ends.

use eframe::egui::{self, Role};
use repose_core::git::activity::{Activity, CommitDay};

use super::{find, of_role};
use crate::state::AppState;
use crate::state::activity::{ActivityRead, DAYS, day_label, local_today, now_secs};
use crate::state::ui_a11y_tests::a11y_frame;
use crate::state::ui_activity_tests::{fleet, path_of, read_as};
use crate::state::ui_harness::frame;

/// An offset west of UTC far enough that the local date is the day before the UTC date now, as
/// in the evening in Chicago.
fn west_of_utc_now(now: i64) -> i32 {
	-(i32::try_from(now.rem_euclid(86_400) / 60).unwrap_or(0) + 30)
}

/// a committed once, now, on the local `today`; b and c read with nothing.
fn mid_scan(now: i64, offset: i32) -> AppState {
	let mut state = fleet();
	let local_day = local_today(now, offset);
	let read = Activity {
		commits: vec![CommitDay {
			time: now,
			local_day,
		}],
		newest: Some(now),
		truncated: false,
	};
	let a = path_of(&state, "a");
	state.activity.reads.insert(a, ActivityRead::Read(read));
	read_as(&mut state, "b", 0, 0);
	read_as(&mut state, "c", 0, 0);
	state.activity.shown = true;
	state.activity.quiet_open = true;
	state.scanning = true;
	state
}

fn cells(ctx: &egui::Context, state: &mut AppState) -> Vec<String> {
	frame(ctx, state, Vec::new());
	let nodes = a11y_frame(ctx, state);
	of_role(&nodes, Role::Cell)
		.into_iter()
		.map(str::to_string)
		.collect()
}

/// Clause 1: the offset not yet in and the scan still running, no cell names a day.
#[test]
fn before_the_offset_is_known_no_cell_names_a_day() {
	let ctx = egui::Context::default();
	let now = now_secs();
	let mut state = mid_scan(now, west_of_utc_now(now));
	state.utc_offset = None;
	let names = cells(&ctx, &mut state);
	assert_eq!(names.len(), 3 * DAYS);
	assert!(
		names.iter().all(|n| n.ends_with(", reading")),
		"{:?}",
		&names[..3]
	);
}

/// Clauses 2 and 3: with the offset in mid-scan and the local date a day behind UTC, the last
/// column is the local today and holds the commit made now; the UTC date is not a column.
#[test]
fn mid_scan_the_last_column_is_the_local_today() {
	let ctx = egui::Context::default();
	let now = now_secs();
	let offset = west_of_utc_now(now);
	let mut state = mid_scan(now, offset);
	state.utc_offset = Some(offset);
	let local = local_today(now, offset);
	assert_eq!(
		local,
		local_today(now, 0) - 1,
		"the fixture's local date trails UTC"
	);
	let today = day_label(DAYS - 1, local).0;
	let names = cells(&ctx, &mut state);
	let nodes = a11y_frame(&ctx, &mut state);
	find(&nodes, Role::Cell, &format!("a, {today}, 1 commit"));
	let utc_today = day_label(DAYS - 1, local + 1).0;
	assert!(
		!names.iter().any(|n| n.contains(&utc_today)),
		"no column for the UTC date {utc_today}"
	);
}

/// When the offset could not be read, the scan's end settles on UTC, as the clocks do.
#[test]
fn a_failed_offset_read_settles_on_utc_when_the_scan_ends() {
	let ctx = egui::Context::default();
	let now = now_secs();
	let mut state = mid_scan(now, 0);
	state.utc_offset = None;
	state.scanning = false;
	let today = day_label(DAYS - 1, local_today(now, 0)).0;
	let nodes = {
		frame(&ctx, &mut state, Vec::new());
		a11y_frame(&ctx, &mut state)
	};
	find(&nodes, Role::Cell, &format!("a, {today}, 1 commit"));
}
