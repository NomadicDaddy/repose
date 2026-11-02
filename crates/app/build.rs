//! Stamps the executable with the commit it was built from and when, for the window's bottom row
//! (Phillip #31631). Git is asked at build time; a build outside a checkout or without git says
//! "unknown" rather than failing. The hash gains "+dirty" when tracked files differ from the
//! commit, so a build of uncommitted work never passes for one of main.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn git(args: &[&str]) -> Option<String> {
	let out = Command::new("git").args(args).output().ok()?;
	if !out.status.success() {
		return None;
	}
	Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() {
	let commit = git(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "unknown".into());
	let dirty = git(&["status", "--porcelain", "--untracked-files=no"])
		.map(|s| !s.is_empty())
		.unwrap_or(false);
	let stamp = if dirty {
		format!("{commit}+dirty")
	} else {
		commit
	};
	let secs = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(|d| d.as_secs())
		.unwrap_or(0);
	println!("cargo:rustc-env=REPOSE_COMMIT={stamp}");
	println!("cargo:rustc-env=REPOSE_BUILT_AT={}", utc_stamp(secs));
	// A new commit or a change to the index changes the stamp; anything else leaves it cached.
	if let Some(dir) = git(&["rev-parse", "--git-dir"]) {
		println!("cargo:rerun-if-changed={dir}/HEAD");
		println!("cargo:rerun-if-changed={dir}/index");
		println!("cargo:rerun-if-changed={dir}/refs/heads");
	}
}

/// `YYYY-MM-DD HH:MM UTC` from seconds since the epoch, with no date crate.
fn utc_stamp(secs: u64) -> String {
	let days = secs / 86_400;
	let rem = secs % 86_400;
	let (h, m) = (rem / 3600, (rem % 3600) / 60);
	// Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
	let z = days as i64 + 719_468;
	let era = z.div_euclid(146_097);
	let doe = z.rem_euclid(146_097);
	let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
	let y = yoe + era * 400;
	let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
	let mp = (5 * doy + 2) / 153;
	let d = doy - (153 * mp + 2) / 5 + 1;
	let mo = if mp < 10 { mp + 3 } else { mp - 9 };
	let y = if mo <= 2 { y + 1 } else { y };
	format!("{y:04}-{mo:02}-{d:02} {h:02}:{m:02} UTC")
}
