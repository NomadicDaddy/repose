//! The bottom row: build information at the right, in the same form as Summon's status bar
//! (Phillip #31631, #56423). The version is the crate's; the commit, build id and build time are
//! stamped by build.rs.

use eframe::egui::{self, RichText, Ui};

use crate::platform::Zone;
use crate::theme::*;

pub const FOOTER_HEIGHT: f32 = 22.0;

/// `repose v1.9.1 cb72b8a · build 3f2a9c01de · 2026-11-17 12:21:04 CST`. A build outside a
/// checkout reads `unknown` for the commit, and one with uncommitted tracked changes carries
/// `+dirty`. The time is the build instant in the zone the last scan read.
pub fn build_info(zone: Option<&Zone>) -> String {
	let secs = env!("REPOSE_BUILT_SECS").parse::<i64>().unwrap_or(0);
	format!(
		"repose v{} {} · build {} · {}",
		env!("CARGO_PKG_VERSION"),
		env!("REPOSE_COMMIT"),
		env!("REPOSE_BUILD_ID"),
		stamp(secs, zone)
	)
}

/// `YYYY-MM-DD HH:MM:SS` and the zone: its initials when known, its offset from UTC when only that
/// is, and UTC itself when nothing is. Every form names the same instant.
fn stamp(secs: i64, zone: Option<&Zone>) -> String {
	let Some(zone) = zone else {
		return format!("{} UTC", civil(secs));
	};
	let local = civil(secs + i64::from(zone.offset_min) * 60);
	match &zone.abbrev {
		Some(abbrev) => format!("{local} {abbrev}"),
		None => {
			let (sign, minutes) = if zone.offset_min < 0 {
				('-', -zone.offset_min)
			} else {
				('+', zone.offset_min)
			};
			format!("{local} UTC{sign}{:02}:{:02}", minutes / 60, minutes % 60)
		}
	}
}

/// Seconds since the epoch to `YYYY-MM-DD HH:MM:SS` (Howard Hinnant's civil-date algorithm).
fn civil(secs: i64) -> String {
	let days = secs.div_euclid(86_400);
	let rem = secs.rem_euclid(86_400);
	let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
	let z = days + 719_468;
	let era = z.div_euclid(146_097);
	let doe = z.rem_euclid(146_097);
	let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
	let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
	let mp = (5 * doy + 2) / 153;
	let d = doy - (153 * mp + 2) / 5 + 1;
	let mo = if mp < 10 { mp + 3 } else { mp - 9 };
	let y = yoe + era * 400 + i64::from(mo <= 2);
	format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02}:{s:02}")
}

pub fn show(root: &mut Ui) {
	let zone = crate::platform::cached_zone();
	egui::Panel::bottom("footer")
		.exact_size(FOOTER_HEIGHT)
		.resizable(false)
		.frame(
			egui::Frame::new()
				.fill(BG_PANEL.get())
				.stroke(egui::Stroke::new(1.0, LINE_SOFT))
				.inner_margin(egui::Margin::symmetric(10, 0)),
		)
		.show(root, |ui| {
			ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
				let text = build_info(zone.as_ref());
				ui.label(RichText::new(text).size(TEXT_SMALL).color(MUTED));
			});
		});
}

#[cfg(test)]
mod tests {
	use super::{Zone, build_info, stamp};

	/// 2026-11-15 05:36:58 UTC, which Summon's footer shows as 2026-11-14 23:36:58 CST.
	const BUILT: i64 = 1_794_721_018;

	fn zone(offset_min: i32, abbrev: Option<&str>) -> Zone {
		Zone {
			offset_min,
			abbrev: abbrev.map(str::to_string),
		}
	}

	#[test]
	fn local_time_with_the_zone_initials() {
		assert_eq!(
			stamp(BUILT, Some(&zone(-360, Some("CST")))),
			"2026-11-14 23:36:58 CST"
		);
	}

	#[test]
	fn an_unnamed_zone_shows_its_offset() {
		assert_eq!(
			stamp(BUILT, Some(&zone(330, None))),
			"2026-11-15 11:06:58 UTC+05:30"
		);
		assert_eq!(
			stamp(BUILT, Some(&zone(-300, None))),
			"2026-11-15 00:36:58 UTC-05:00"
		);
	}

	#[test]
	fn an_unknown_zone_shows_utc() {
		assert_eq!(stamp(BUILT, None), "2026-11-15 05:36:58 UTC");
	}

	#[test]
	fn the_line_reads_like_summons() {
		let line = build_info(Some(&zone(-360, Some("CST"))));
		let version = format!("repose v{} ", env!("CARGO_PKG_VERSION"));
		assert!(line.starts_with(&version), "{line}");
		let parts: Vec<&str> = line.split(" · ").collect();
		assert_eq!(parts.len(), 3, "{line}");
		let id = parts[1].strip_prefix("build ").unwrap_or_default();
		assert_eq!(id.len(), 10, "{line}");
		assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "{line}");
		assert!(parts[2].ends_with(" CST"), "{line}");
	}
}
