//! `--measure-lanes <file>` (branch-graph-lanes clause 4): with lanes on and History open, wait
//! for the first page, take idle frames to find the display's refresh interval, then turn the
//! wheel over the History pane one row a frame, 50 rows down and 50 back up, a repaint every
//! frame, and write what each frame cost. Real wheel events go in through eframe's raw-input
//! hook, so the lanes draw the way a user's scroll draws them. Only frames in which the pane's
//! scroll offset moved are counted, so frames redrawing an unchanged view cannot pad the
//! figures (Morticia L-1 on c70d37d).
//!
//! Two series per counted frame: the present-to-present interval, which covers everything the
//! frame cost including tessellation and the GPU submit after `ui()`, and the CPU time inside
//! `ui()` alone.
//!
//! The pass rule, fixed before any run (Morticia #39826): at least 70 counted frames, the p95
//! `ui()` time under the refresh interval, and the p95 present interval within 10% of it.
//! Frames over 1.5 intervals are counted as missed and reported either way.

use std::path::{Path, PathBuf};
use std::time::Instant;

use eframe::egui::{self, Event, MouseWheelUnit, RawInput, pos2, vec2};

use crate::state::AppState;
use crate::theme::{COMMIT_ROW_HEIGHT, SIDEBAR_WIDTH};

/// Frames with no input, to read the refresh interval from.
const IDLE_FRAMES: usize = 60;
/// Frames of wheel input: one row each, half down and half back up, within a first page of 100.
const SCROLL_FRAMES: usize = 100;
/// Counted (moving) frames a valid run needs.
const MIN_MOVING: usize = 70;
/// The page has not landed after this long: the run is written as failed.
const WAIT_LIMIT_S: u64 = 120;

enum Phase {
	Waiting,
	Idle,
	Scrolling,
}

pub struct LanesProbe {
	out: PathBuf,
	phase: Phase,
	started: Instant,
	first_history: Option<Instant>,
	frame_start: Instant,
	last_start: Option<Instant>,
	idle: Vec<f64>,
	present: Vec<f64>,
	cpu: Vec<f64>,
	page: Vec<(&'static str, String)>,
	/// Wheel frames so far, counted or not.
	scrolled: usize,
	last_offset: Option<f32>,
	/// This frame's present interval, kept until the frame shows whether it moved.
	pending: Option<f64>,
}

impl LanesProbe {
	pub fn new(out: PathBuf) -> Self {
		Self {
			out,
			phase: Phase::Waiting,
			started: Instant::now(),
			first_history: None,
			frame_start: Instant::now(),
			last_start: None,
			idle: Vec::new(),
			present: Vec::new(),
			cpu: Vec::new(),
			page: Vec::new(),
			scrolled: 0,
			last_offset: None,
			pending: None,
		}
	}

	pub fn out(&self) -> &Path {
		&self.out
	}

	/// While scrolling: the pointer over the History pane and one row's turn of the wheel.
	pub fn inject(&self, raw: &mut RawInput) {
		if !matches!(self.phase, Phase::Scrolling) {
			return;
		}
		let screen = raw.screen_rect.unwrap_or(egui::Rect::ZERO);
		let at = pos2(
			(screen.left() + SIDEBAR_WIDTH + screen.right()) / 2.0,
			screen.center().y,
		);
		// A negative delta scrolls down; the second half comes back up.
		let row = COMMIT_ROW_HEIGHT + 6.0;
		let delta = if self.scrolled < SCROLL_FRAMES / 2 {
			-row
		} else {
			row
		};
		raw.events.push(Event::PointerMoved(at));
		raw.events.push(Event::MouseWheel {
			unit: MouseWheelUnit::Point,
			delta: vec2(0.0, delta),
			phase: egui::TouchPhase::Move,
			modifiers: egui::Modifiers::default(),
			source: egui::MouseWheelSource::Wheel,
		});
	}

	/// At the start of `ui()`: the interval since the last frame began.
	pub fn begin(&mut self) {
		let now = Instant::now();
		if let Some(last) = self.last_start {
			let ms = ms(now - last);
			match self.phase {
				Phase::Idle => self.idle.push(ms),
				Phase::Scrolling => self.pending = Some(ms),
				Phase::Waiting => {}
			}
		}
		self.last_start = Some(now);
		self.frame_start = now;
	}

	/// At the end of `ui()`: the frame's CPU time, the next phase, and the report once done.
	pub fn end(&mut self, state: &AppState, ctx: &egui::Context) -> Option<String> {
		ctx.request_repaint();
		let cpu = ms(self.frame_start.elapsed());
		match self.phase {
			Phase::Waiting => self.wait(state, cpu),
			Phase::Idle => {
				if self.idle.len() >= IDLE_FRAMES {
					self.phase = Phase::Scrolling;
				}
				None
			}
			Phase::Scrolling => {
				self.scrolled += 1;
				let offset = crate::screens::detail::scroll_offset(ctx);
				let moved = self.last_offset.is_some() && offset != self.last_offset;
				self.last_offset = offset;
				let present = self.pending.take();
				if moved && let Some(present) = present {
					self.present.push(present);
					self.cpu.push(cpu);
				}
				(self.scrolled >= SCROLL_FRAMES).then(|| self.report())
			}
		}
	}

	fn wait(&mut self, state: &AppState, cpu: f64) -> Option<String> {
		// First, so a run whose repository never selects still ends.
		if self.started.elapsed().as_secs() > WAIT_LIMIT_S {
			return Some(format!("error\tno lanes page within {WAIT_LIMIT_S} s\n"));
		}
		let repo = state.selected_repo()?.path.clone();
		let first = *self.first_history.get_or_insert_with(Instant::now);
		let h = state.history_of(&repo)?;
		if let Some(e) = &h.error {
			return Some(format!("error\t{e}\n"));
		}
		if h.graph.is_empty() {
			return None;
		}
		let (git, layout) = h.timing.unwrap_or_default();
		let widest = h.graph.iter().map(|r| r.width).max().unwrap_or(0);
		self.page = vec![
			("repo", repo.display().to_string()),
			("commits", h.commits.len().to_string()),
			("widest_lanes", widest.to_string()),
			("first_page_ms", fmt(ms(first.elapsed()))),
			("git_log_ms", fmt(git)),
			("layout_ms", fmt(layout)),
			("first_draw_ms", fmt(cpu)),
		];
		self.phase = Phase::Idle;
		None
	}

	fn report(&self) -> String {
		report(
			&self.page,
			&self.idle,
			self.scrolled,
			&self.present,
			&self.cpu,
		)
	}
}

fn ms(d: std::time::Duration) -> f64 {
	d.as_secs_f64() * 1000.0
}

fn fmt(v: f64) -> String {
	format!("{v:.2}")
}

/// The value at `p` (0 to 1) of `values` sorted, nearest rank.
fn percentile(values: &[f64], p: f64) -> f64 {
	let mut v = values.to_vec();
	v.sort_by(f64::total_cmp);
	let rank = ((p * v.len() as f64).ceil() as usize).clamp(1, v.len().max(1));
	v.get(rank - 1).copied().unwrap_or(0.0)
}

/// The tab-separated report: the page lines, the refresh interval, the wheel frames and how
/// many of them moved, both counted series as p50, p95 and max, the missed frames, and the
/// verdict under the module's pass rule.
fn report(
	page: &[(&str, String)],
	idle: &[f64],
	frames: usize,
	present: &[f64],
	cpu: &[f64],
) -> String {
	let refresh = percentile(idle, 0.5);
	let stats = |v: &[f64]| {
		let max = v.iter().copied().fold(0.0, f64::max);
		let p = |q| fmt(percentile(v, q));
		format!("{}\t{}\t{}", p(0.5), p(0.95), fmt(max))
	};
	let missed = present.iter().filter(|&&p| p > refresh * 1.5).count();
	let pass = present.len() >= MIN_MOVING
		&& percentile(cpu, 0.95) < refresh
		&& percentile(present, 0.95) <= refresh * 1.1;
	let mut out: String = page.iter().map(|(k, v)| format!("{k}\t{v}\n")).collect();
	out.push_str(&format!("refresh_ms\t{}\n", fmt(refresh)));
	out.push_str(&format!("wheel_frames\t{frames}\n"));
	out.push_str(&format!("moving_frames\t{}\n", present.len()));
	out.push_str(&format!("present_ms_p50_p95_max\t{}\n", stats(present)));
	out.push_str(&format!("ui_cpu_ms_p50_p95_max\t{}\n", stats(cpu)));
	out.push_str(&format!("missed\t{missed}\n"));
	out.push_str(&format!("pass\t{}\n", if pass { "yes" } else { "no" }));
	out
}

#[cfg(test)]
mod tests {
	use super::{percentile, report};

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
}
