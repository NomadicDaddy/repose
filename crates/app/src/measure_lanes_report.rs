//! The `--measure-lanes` report: nearest-rank percentiles, the tab-separated lines and the
//! verdict under the pass rule stated in measure_lanes.rs.

use super::MIN_MOVING;

pub(super) fn fmt(v: f64) -> String {
	format!("{v:.2}")
}

/// The value at `p` (0 to 1) of `values` sorted, nearest rank.
pub(super) fn percentile(values: &[f64], p: f64) -> f64 {
	let mut v = values.to_vec();
	v.sort_by(f64::total_cmp);
	let rank = ((p * v.len() as f64).ceil() as usize).clamp(1, v.len().max(1));
	v.get(rank - 1).copied().unwrap_or(0.0)
}

/// The tab-separated report: the page lines, the refresh interval, the wheel frames and how
/// many of them moved, both counted series as p50, p95 and max, the missed frames, and the
/// verdict under the module's pass rule.
pub(super) fn report(
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
	let pass = cpu.len() >= MIN_MOVING
		&& present.len() >= MIN_MOVING
		&& percentile(cpu, 0.95) < refresh
		&& percentile(present, 0.95) <= refresh * 1.1;
	let mut out: String = page.iter().map(|(k, v)| format!("{k}\t{v}\n")).collect();
	out.push_str(&format!("refresh_ms\t{}\n", fmt(refresh)));
	out.push_str(&format!("wheel_frames\t{frames}\n"));
	out.push_str(&format!("moving_frames\t{}\n", cpu.len()));
	out.push_str(&format!("aligned_intervals\t{}\n", present.len()));
	out.push_str(&format!(
		"frame_interval_ms_p50_p95_max\t{}\n",
		stats(present)
	));
	out.push_str(&format!("ui_cpu_ms_p50_p95_max\t{}\n", stats(cpu)));
	out.push_str(&format!("missed\t{missed}\n"));
	out.push_str(&format!("pass\t{}\n", if pass { "yes" } else { "no" }));
	out
}
