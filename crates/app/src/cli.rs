//! The command-line surface that main.rs reads: the help text and the --view matcher, kept here so
//! a test can call them (cli-help-and-view-validation).

use crate::state::View;

/// Printed to stderr by `--help` or `-h`, which then exits 0 before any window opens.
pub const HELP: &str = "\
repose: a desktop viewer for the Git repositories under your configured roots.

Usage: repose [flags]

  --config <path>                     Use this config file instead of %APPDATA%\\repose\\config.toml.
  --select <path>                     Open on this repository.
  --view changes|history|files|branches
                                      Open on this pane (default changes).
  --file <path>                       Open this file's working-tree diff.
  --inline                            Show that diff inline rather than side by side.
  --open <path>                       Open this file read-only, as HEAD holds it.
  --scale <factor>                    Zoom the window by this factor, 0.5 through 4.
  --measure-startup [file]            Write the milliseconds to the first frame with rows, then exit.
  --measure-diff <file>               Write a line once the open diff has loaded, and keep running.
  -h, --help                          Print this help and exit.
";

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
mod tests {
	use super::*;

	#[test]
	fn view_accepts_the_four_panes_and_refuses_anything_else() {
		assert_eq!(view(None), Ok(View::Changes));
		assert_eq!(view(Some("changes")), Ok(View::Changes));
		assert_eq!(view(Some("history")), Ok(View::History));
		assert_eq!(view(Some("files")), Ok(View::Files));
		assert_eq!(view(Some("branches")), Ok(View::Branches));
		let refused = match view(Some("brnaches")) {
			Err(message) => message,
			Ok(view) => panic!("a misspelt pane opened {view:?}"),
		};
		assert!(
			refused.contains("changes, history, files or branches"),
			"{refused}"
		);
		assert!(refused.contains("\"brnaches\""), "{refused}");
		assert!(
			view(Some("Branches")).is_err(),
			"the names are exact, not case-folded"
		);
		assert!(
			view(Some("")).is_err(),
			"an empty value is refused, not Changes"
		);
	}

	#[test]
	fn help_names_every_flag_main_reads() {
		for flag in [
			"--config <path>",
			"--select <path>",
			"--view changes|history|files|branches",
			"--file <path>",
			"--inline",
			"--open <path>",
			"--scale <factor>",
			"0.5 through 4",
			"--measure-startup [file]",
			"--measure-diff <file>",
			"-h, --help",
		] {
			assert!(HELP.contains(flag), "help is missing {flag}");
		}
	}
}
