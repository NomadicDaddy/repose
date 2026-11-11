//! The Activity screen's state (activity-screen): whether it is shown, the tile filter, and each
//! repository's commit activity, read on background threads under a serial so a stale read from
//! an earlier showing is dropped. The day buckets, colour steps, quiet test and tile counts are
//! pure functions here, so the screen draws what they decide and the tests pin them.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui;
use repose_core::git::activity::{Activity, CommitDay, read_activity};
use repose_core::model::RepoSummary;

use super::{AppState, Msg};

/// Days in the grid, ending today.
pub const DAYS: usize = 30;
/// A repository with no commit in this many days counts as quiet.
pub const QUIET_DAYS: i64 = 60;
/// Reads at once; activity is a short `git log` per repository.
const WORKERS: usize = 4;
const DAY: i64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
	Uncommitted,
	Ahead,
	Behind,
	Quiet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityRead {
	Loading,
	Read(Activity),
	Failed(String),
}

pub struct ActivityMsg {
	serial: u64,
	path: PathBuf,
	result: Result<Activity, String>,
}

#[derive(Default)]
pub struct ActivityState {
	pub shown: bool,
	pub filter: Option<Tile>,
	pub reads: HashMap<PathBuf, ActivityRead>,
	/// The quiet repositories' closing row is open.
	pub quiet_open: bool,
	/// The grid's focused cell: (row in drawn order, day index), the day None on the row's name.
	pub cell: (usize, Option<usize>),
	/// The grid widget's id as last drawn, its one Tab stop.
	pub grid_id: Option<egui::Id>,
	serial: u64,
	/// The serial the workers check before each read, so a superseded run stops early (Morticia
	/// L-2 on 4051085).
	live: Arc<AtomicU64>,
}

/// Today's local date as days since 1970-01-01; `offset_min` is the current minutes east of UTC.
pub fn local_today(now: i64, offset_min: i32) -> i64 {
	(now + i64::from(offset_min) * 60).div_euclid(DAY)
}

/// Commits per local day for the `DAYS` days ending on `today`; index `DAYS - 1` is today. Each
/// commit's local date is git's, taken with the zone rule at its own instant.
pub fn day_counts(commits: &[CommitDay], today: i64) -> [u32; DAYS] {
	let mut out = [0u32; DAYS];
	for c in commits {
		let back = today - c.local_day;
		if (0..DAYS as i64).contains(&back) {
			out[DAYS - 1 - back as usize] += 1;
		}
	}
	out
}

/// The colour step for a day's commit count: 0 for none, then 1 to 4 for 1-3, 4-7, 8-15 and 16+.
pub fn step(count: u32) -> usize {
	match count {
		0 => 0,
		1..=3 => 1,
		4..=7 => 2,
		8..=15 => 3,
		_ => 4,
	}
}

/// Quiet: no commit at all, or the newest is at least `QUIET_DAYS` days old.
pub fn is_quiet(newest: Option<i64>, now: i64) -> bool {
	newest.is_none_or(|t| now - t >= QUIET_DAYS * DAY)
}

/// Whether `repo` belongs to a tile, given its activity read when the tile needs it.
pub fn in_tile(tile: Tile, repo: &RepoSummary, read: Option<&ActivityRead>, now: i64) -> bool {
	let status = repo.status.as_ref();
	match tile {
		Tile::Uncommitted => status.is_some_and(|s| s.changed() > 0),
		Tile::Ahead => status.and_then(|s| s.ahead).is_some_and(|n| n > 0),
		Tile::Behind => status.and_then(|s| s.behind).is_some_and(|n| n > 0),
		Tile::Quiet => matches!(read, Some(ActivityRead::Read(a)) if is_quiet(a.newest, now)),
	}
}

impl AppState {
	/// The start-up rule (clause 2): a repository asked for on the command line, or the one the
	/// working folder is inside, opens as before; with neither, the pane opens on Activity.
	pub fn start_on(&mut self, selection: Option<PathBuf>) {
		self.activity.shown = selection.is_none();
		self.requested_selection = selection;
	}

	/// Shows the Activity screen and reads every listed repository's activity afresh.
	pub fn show_activity(&mut self, ctx: &egui::Context) {
		self.activity.shown = true;
		self.read_activity(ctx);
	}

	/// Reads the activity of every top-level repository (a worktree shares its repository's
	/// history) on `WORKERS` threads; results land through `merge_activity`.
	pub fn read_activity(&mut self, ctx: &egui::Context) {
		self.activity.serial += 1;
		let serial = self.activity.serial;
		self.activity.live.store(serial, Ordering::Relaxed);
		let since = now_secs() - DAYS as i64 * DAY - DAY;
		let paths: VecDeque<PathBuf> = self.repos.iter().map(|r| r.path.clone()).collect();
		for p in &paths {
			self.activity.reads.insert(p.clone(), ActivityRead::Loading);
		}
		let queue = Arc::new(Mutex::new(paths));
		for _ in 0..WORKERS {
			let queue = Arc::clone(&queue);
			let live = Arc::clone(&self.activity.live);
			let tx = self.sender();
			let ctx = ctx.clone();
			std::thread::spawn(move || {
				while live.load(Ordering::Relaxed) == serial {
					let next = queue.lock().ok().and_then(|mut q| q.pop_front());
					let Some(path) = next else { break };
					let result = read_activity(&path, since).map_err(|e| e.to_string());
					let _ = tx.send(Msg::Activity(ActivityMsg {
						serial,
						path,
						result,
					}));
					ctx.request_repaint();
				}
			});
		}
	}

	pub(super) fn merge_activity(&mut self, msg: ActivityMsg) {
		if msg.serial != self.activity.serial {
			return;
		}
		let read = match msg.result {
			Ok(a) => ActivityRead::Read(a),
			Err(e) => ActivityRead::Failed(e),
		};
		self.activity.reads.insert(msg.path, read);
	}
}

pub fn now_secs() -> i64 {
	std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map(|d| d.as_secs() as i64)
		.unwrap_or(0)
}

/// The grid column `index` (0 is the oldest, `DAYS - 1` today) as a local date label, "Tue 4 Nov",
/// and its day of the month for the column header.
pub fn day_label(index: usize, today: i64) -> (String, u32) {
	const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
	const MONTHS: [&str; 12] = [
		"Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
	];
	let days = today - (DAYS - 1 - index) as i64;
	// Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
	let z = days + 719_468;
	let era = z.div_euclid(146_097);
	let doe = z - era * 146_097;
	let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
	let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
	let mp = (5 * doy + 2) / 153;
	let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
	let month = if mp < 10 { mp + 3 } else { mp - 9 } as usize;
	let weekday = WEEKDAYS[days.rem_euclid(7) as usize];
	(format!("{weekday} {day} {}", MONTHS[month - 1]), day)
}

#[cfg(test)]
mod tests {
	use repose_core::git::activity::CommitDay;

	use super::{DAY, DAYS, day_counts, day_label, is_quiet, local_today, step};

	#[test]
	fn day_labels_name_the_local_date() {
		// 2026-11-11 is a Wednesday, 20,768 days after 1970-01-01.
		assert_eq!(day_label(DAYS - 1, 20_768), ("Wed 11 Nov".to_string(), 11));
		assert_eq!(day_label(DAYS - 12, 20_768), ("Sat 31 Oct".to_string(), 31));
	}

	/// clause 4: counts land on their local days, the last index is today, and older commits drop
	/// out. Today follows the current offset: 01:00 UTC is still yesterday six hours west.
	#[test]
	fn commits_land_on_their_local_day() {
		let now = 1_000 * DAY + 3_600;
		let today = local_today(now, 0);
		assert_eq!(today, 1_000);
		assert_eq!(local_today(now, -360), 999);
		let on = |local_day| CommitDay { time: 0, local_day };
		let counts = day_counts(&[on(1_000), on(999), on(999), on(960)], today);
		assert_eq!(counts[DAYS - 1], 1, "today");
		assert_eq!(counts[DAYS - 2], 2, "yesterday");
		assert_eq!(
			counts.iter().sum::<u32>(),
			3,
			"40 days ago is outside the grid"
		);
	}

	#[test]
	fn five_steps_with_their_thresholds() {
		let steps: Vec<usize> = [0, 1, 3, 4, 7, 8, 15, 16, 99].map(step).to_vec();
		assert_eq!(steps, vec![0, 1, 1, 2, 2, 3, 3, 4, 4]);
	}

	#[test]
	fn quiet_means_no_commit_or_sixty_days_old() {
		let now = 1_000 * DAY;
		assert!(is_quiet(None, now));
		assert!(is_quiet(Some(now - 60 * DAY), now));
		assert!(!is_quiet(Some(now - 59 * DAY), now));
	}
}
