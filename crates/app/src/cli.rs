//! The command-line surface: one table of flags that both `--help` and the parser read, so a flag
//! cannot be read without a help line (cli-help-and-view-validation; the flag-parsing tidy-up).

use std::cell::RefCell;
use std::path::PathBuf;

use crate::state::View;

/// One flag as `--help` prints it and the parser looks it up.
pub struct Flag {
	pub name: &'static str,
	/// A second spelling the parser also accepts, printed before the name ("" for none).
	pub alias: &'static str,
	/// What follows the name in the help, such as `<path>` ("" for none).
	pub value: &'static str,
	pub help: &'static str,
}

/// Every flag repose reads, in the order `--help` lists them.
pub const FLAGS: &[Flag] = &[
	flag(
		"--config",
		"<path>",
		"Use this config file instead of %APPDATA%\\repose\\config.toml.",
	),
	flag("--select", "<path>", "Open on this repository."),
	flag(
		"--view",
		"changes|history|files|branches",
		"Open on this pane (default changes).",
	),
	flag("--file", "<path>", "Open this file's working-tree diff."),
	flag(
		"--inline",
		"",
		"Show that diff inline rather than side by side.",
	),
	flag(
		"--lanes",
		"",
		"Open with lanes on in History, for the session.",
	),
	flag(
		"--open",
		"<path>",
		"Open this file read-only, as HEAD holds it.",
	),
	flag(
		"--scale",
		"<factor>",
		"Zoom the window by this factor, 0.5 through 4.",
	),
	flag(
		"--measure-startup",
		"[file]",
		"Write the milliseconds to the first frame with rows, then exit.",
	),
	flag(
		"--measure-diff",
		"<file>",
		"Write a line once the open diff has loaded, and keep running.",
	),
	flag(
		"--measure-lanes",
		"<file>",
		"With --view history: turn lanes on, scroll 100 rows, write frame times, then exit.",
	),
	Flag {
		name: "--help",
		alias: "-h",
		value: "",
		help: "Print this help and exit.",
	},
];

const fn flag(name: &'static str, value: &'static str, help: &'static str) -> Flag {
	Flag {
		name,
		alias: "",
		value,
		help,
	}
}

/// The column the help text starts in; a longer flag puts its text on the next line.
const HELP_COLUMN: usize = 38;

/// Printed to stderr by `--help` or `-h`, which then exits 0 before any window opens.
pub fn help() -> String {
	let mut out = String::from(
		"repose: a desktop viewer for the Git repositories under your configured roots.\n\n\
		 Usage: repose [flags]\n\n",
	);
	for f in FLAGS {
		let mut left = String::from("  ");
		if !f.alias.is_empty() {
			left.push_str(f.alias);
			left.push_str(", ");
		}
		left.push_str(f.name);
		if !f.value.is_empty() {
			left.push(' ');
			left.push_str(f.value);
		}
		if left.len() >= HELP_COLUMN - 2 {
			left.push('\n');
			left.push_str(&" ".repeat(HELP_COLUMN));
		} else {
			left.push_str(&" ".repeat(HELP_COLUMN - left.len()));
		}
		out.push_str(&left);
		out.push_str(f.help);
		out.push('\n');
	}
	out
}

/// What the command line asks for.
#[derive(Debug, PartialEq)]
pub enum Parsed {
	Help,
	Run(Args),
	/// A flag value repose refuses, with the message main.rs shows before exiting 2.
	Refused(String),
}

/// The flags main.rs applies. Defaults that read the machine (the config path, the repository
/// at the current folder) are left to main.rs.
#[derive(Debug, PartialEq)]
pub struct Args {
	pub config: Option<PathBuf>,
	pub select: Option<PathBuf>,
	pub view: View,
	pub file: Option<String>,
	pub inline: bool,
	pub lanes: bool,
	pub open: Option<String>,
	pub scale: Option<f32>,
	pub measure_startup: Option<PathBuf>,
	pub measure_diff: Option<PathBuf>,
	pub measure_lanes: Option<PathBuf>,
}

pub fn parse(args: &[String]) -> Parsed {
	parse_with(&Lookup::new(args))
}

/// Reads the command line, only through flags in [`FLAGS`], and remembers which it read.
struct Lookup<'a> {
	args: &'a [String],
	read: RefCell<Vec<&'static str>>,
}

impl<'a> Lookup<'a> {
	fn new(args: &'a [String]) -> Self {
		Self {
			args,
			read: RefCell::new(Vec::new()),
		}
	}

	/// The index of the flag, which must have a [`FLAGS`] entry. Every lookup runs on every start,
	/// so a flag read without one fails the first test that parses a command line.
	fn index(&self, name: &str) -> Option<usize> {
		let Some(flag) = FLAGS.iter().find(|f| f.name == name) else {
			panic!("{name} has no FLAGS entry, so --help would not name it");
		};
		self.read.borrow_mut().push(flag.name);
		let named = |a: &String| a == flag.name || (!flag.alias.is_empty() && a == flag.alias);
		self.args.iter().skip(1).position(named).map(|i| i + 1)
	}

	fn present(&self, name: &str) -> bool {
		self.index(name).is_some()
	}

	/// The word after the flag: outer None when the flag is absent, inner None when it ends the line.
	fn value(&self, name: &str) -> Option<Option<&'a String>> {
		self.index(name).map(|i| self.args.get(i + 1))
	}
}

fn parse_with(look: &Lookup<'_>) -> Parsed {
	if look.present("--help") {
		return Parsed::Help;
	}
	// --view with no value after it is refused too, as an empty name, rather than opening Changes.
	let view = match view(look.value("--view").map(|v| v.map_or("", String::as_str))) {
		Ok(view) => view,
		Err(message) => return Parsed::Refused(message),
	};
	// A zoom factor multiplied into the desktop's own scaling. A value that is not a number
	// between 0.5 and 4 is refused out loud rather than run at normal zoom without a word
	// (Morticia Info on 4b39403).
	let scale = match look.value("--scale") {
		None => None,
		Some(given) => match given.and_then(|s| s.parse::<f32>().ok()) {
			Some(s) if (0.5..=4.0).contains(&s) => Some(s),
			_ => {
				let given = given.cloned().unwrap_or_default();
				return Parsed::Refused(format!(
					"--scale needs a number between 0.5 and 4, not {given:?}"
				));
			}
		},
	};
	let path = |name| look.value(name).flatten().map(PathBuf::from);
	let text = |name| look.value(name).flatten().cloned();
	Parsed::Run(Args {
		config: path("--config"),
		// `.` and relative paths resolve against the current folder; the raw value is kept if
		// that fails.
		select: text("--select").map(|s| std::path::absolute(&s).unwrap_or_else(|_| s.into())),
		view,
		file: text("--file"),
		inline: look.present("--inline"),
		lanes: look.present("--lanes"),
		open: text("--open"),
		scale,
		measure_startup: look
			.value("--measure-startup")
			.map(|v| PathBuf::from(v.map_or("repose-startup-ms.txt", String::as_str))),
		measure_diff: path("--measure-diff"),
		measure_lanes: path("--measure-lanes"),
	})
}

/// The pane `--view` names: none means Changes, and anything but the four names is refused with
/// the message main.rs shows.
pub fn view(value: Option<&str>) -> Result<View, String> {
	match value {
		None | Some("changes") => Ok(View::Changes),
		Some("history") => Ok(View::History),
		Some("files") => Ok(View::Files),
		Some("branches") => Ok(View::Branches),
		Some(other) => Err(format!(
			"--view needs one of changes, history, files or branches, not {other:?}"
		)),
	}
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
