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
	// The footer shows this instant in the viewer's local time, as Summon's does (Phillip #56423).
	println!("cargo:rustc-env=REPOSE_BUILT_SECS={secs}");
	println!("cargo:rustc-env=REPOSE_BUILD_ID={}", build_id(&INPUTS));
	// The build id also covers the manifests, the lockfile and the toolchain pin, so a change to
	// any of them restamps it; the sources, syntaxes and icon are named below.
	for input in &INPUTS[5..] {
		println!("cargo:rerun-if-changed={input}");
	}
	// Naming any rerun path turns off cargo's default of rerunning on every source change, so
	// the crate's own sources are named too: an edit to a tracked file then restamps "+dirty"
	// and a fresh time instead of keeping the old clean stamp (Morticia Low on c1fb690).
	println!("cargo:rerun-if-changed=src");
	println!("cargo:rerun-if-changed=build.rs");
	// The executable also compiles repose-core and embeds the vendored syntaxes with include_str!,
	// so an edit to either must restamp as well.
	println!("cargo:rerun-if-changed=../core/src");
	println!("cargo:rerun-if-changed=syntaxes");
	if let Some(dir) = git(&["rev-parse", "--git-dir"]) {
		println!("cargo:rerun-if-changed={dir}/HEAD");
		println!("cargo:rerun-if-changed={dir}/index");
		println!("cargo:rerun-if-changed={dir}/refs/heads");
	}
	program_icon();
}

/// The program icon (program-icon): on a Windows target the executable carries repose.ico as its
/// icon resource, which Explorer, the taskbar, Alt+Tab and the Start menu show. Other targets build
/// without it. A failure to compile the resource fails the build rather than ship an exe with none.
fn program_icon() {
	const ICON: &str = "../../assets/icon/repose.ico";
	// The whole folder: main.rs also embeds repose-256.png, so an edit to either file must restamp
	// the build, as for the crate's own sources (Morticia L-1 on 1e170f7).
	println!("cargo:rerun-if-changed=../../assets/icon");
	if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
		return;
	}
	if let Err(e) = winresource::WindowsResource::new().set_icon(ICON).compile() {
		panic!("could not compile the program icon {ICON}: {e}");
	}
}

/// What the executable is built from, relative to this crate: its sources, repose-core's, the
/// vendored syntaxes and the icon (already watched in main), then the manifests, the lockfile and
/// the toolchain pin.
const INPUTS: [&str; 10] = [
	"src",
	"build.rs",
	"syntaxes",
	"../core/src",
	"../../assets/icon",
	"Cargo.toml",
	"../core/Cargo.toml",
	"../../Cargo.toml",
	"../../Cargo.lock",
	"../../rust-toolchain.toml",
];

/// The footer's `build` id, 10 hex digits as Summon's is: a fingerprint of every input file's path
/// and bytes in sorted order, so two builds from different content never share one, even at the
/// same commit. std's hasher needs no dependency; its algorithm may change between Rust releases,
/// which the toolchain pin holds still.
fn build_id(inputs: &[&str]) -> String {
	use std::hash::{DefaultHasher, Hasher};
	let mut files = Vec::new();
	for input in inputs {
		collect(std::path::Path::new(input), &mut files);
	}
	files.sort();
	let mut hasher = DefaultHasher::new();
	for file in &files {
		hasher.write(file.to_string_lossy().replace('\\', "/").as_bytes());
		hasher.write(&std::fs::read(file).unwrap_or_default());
	}
	format!("{:016x}", hasher.finish())[..10].to_string()
}

fn collect(path: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
	if path.is_file() {
		files.push(path.to_path_buf());
	} else if let Ok(entries) = std::fs::read_dir(path) {
		for entry in entries.flatten() {
			collect(&entry.path(), files);
		}
	}
}
