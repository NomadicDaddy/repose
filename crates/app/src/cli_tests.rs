use super::*;

fn line(words: &[&str]) -> Vec<String> {
	std::iter::once("repose")
		.chain(words.iter().copied())
		.map(String::from)
		.collect()
}

fn run(words: &[&str]) -> Args {
	match parse(&line(words)) {
		Parsed::Run(args) => args,
		other => panic!("{words:?} gave {other:?}"),
	}
}

fn refused(words: &[&str]) -> String {
	match parse(&line(words)) {
		Parsed::Refused(message) => message,
		other => panic!("{words:?} gave {other:?}"),
	}
}

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

/// The parser reads every flag in the table and nothing else (L-1 on e556624): a lookup of a name
/// with no entry panics, and an entry the parser never reads fails here.
#[test]
fn the_parser_reads_exactly_the_flags_help_names() {
	for words in [&[][..], &["--view", "history", "--scale", "1.25"][..]] {
		let args = line(words);
		let look = Lookup::new(&args);
		assert!(matches!(parse_with(&look), Parsed::Run(_)));
		let read = look.read.borrow();
		for f in FLAGS {
			assert!(
				read.contains(&f.name),
				"{} is in --help but never read",
				f.name
			);
		}
	}
}

#[test]
#[should_panic(expected = "--unlisted has no FLAGS entry")]
fn a_flag_without_a_table_entry_cannot_be_read() {
	let args = line(&[]);
	let _ = Lookup::new(&args).present("--unlisted");
}

#[test]
fn help_prints_every_table_entry_on_its_own_line() {
	let help = help();
	for f in FLAGS {
		let usage = if f.value.is_empty() {
			f.name.to_string()
		} else {
			format!("{} {}", f.name, f.value)
		};
		assert!(help.contains(&usage), "help is missing {usage}");
		assert!(
			help.contains(f.help),
			"help is missing the text for {}",
			f.name
		);
	}
	assert!(help.contains("  -h, --help"), "{help}");
	assert!(help.contains("0.5 through 4"), "{help}");
	assert!(
		help.contains("  --config <path>                     Use this config file"),
		"{help}"
	);
	assert!(
		help.contains("  --view changes|history|files|branches\n      "),
		"a long flag puts its text on the next line: {help}"
	);
}

#[test]
fn help_wins_over_a_refused_value() {
	assert_eq!(parse(&line(&["--view", "nope", "-h"])), Parsed::Help);
	assert_eq!(parse(&line(&["--help"])), Parsed::Help);
}

#[test]
fn no_flags_gives_the_defaults() {
	assert_eq!(
		run(&[]),
		Args {
			config: None,
			select: None,
			view: View::Changes,
			file: None,
			inline: false,
			open: None,
			scale: None,
			measure_startup: None,
			measure_diff: None,
		}
	);
}

#[test]
fn every_flag_carries_its_value() {
	let a = run(&[
		"--config",
		"c.toml",
		"--select",
		"D:\\r",
		"--view",
		"files",
		"--file",
		"a.txt",
		"--inline",
		"--open",
		"b.txt",
		"--scale",
		"1.25",
		"--measure-startup",
		"m.txt",
		"--measure-diff",
		"d.txt",
	]);
	assert_eq!(a.config, Some(PathBuf::from("c.toml")));
	assert_eq!(a.select, Some(PathBuf::from("D:\\r")));
	assert_eq!(a.view, View::Files);
	assert_eq!(a.file.as_deref(), Some("a.txt"));
	assert!(a.inline);
	assert_eq!(a.open.as_deref(), Some("b.txt"));
	assert_eq!(a.scale, Some(1.25));
	assert_eq!(a.measure_startup, Some(PathBuf::from("m.txt")));
	assert_eq!(a.measure_diff, Some(PathBuf::from("d.txt")));
}

#[test]
fn measure_startup_without_a_file_uses_the_default_name() {
	assert_eq!(
		run(&["--measure-startup"]).measure_startup,
		Some(PathBuf::from("repose-startup-ms.txt"))
	);
}

#[test]
fn a_relative_select_resolves_against_the_current_folder() {
	let select = run(&["--select", "."]).select;
	assert!(
		select.as_deref().is_some_and(std::path::Path::is_absolute),
		"{select:?}"
	);
}

#[test]
fn bad_values_are_refused_with_the_value_named() {
	assert!(refused(&["--view"]).contains("not \"\""));
	assert!(refused(&["--view", "brnaches"]).contains("\"brnaches\""));
	for bad in ["0.4", "4.1", "big"] {
		let message = refused(&["--scale", bad]);
		assert!(message.contains("between 0.5 and 4"), "{message}");
		assert!(message.contains(&format!("{bad:?}")), "{message}");
	}
	assert!(refused(&["--scale"]).contains("not \"\""));
	assert_eq!(run(&["--scale", "0.5"]).scale, Some(0.5));
	assert_eq!(run(&["--scale", "4"]).scale, Some(4.0));
}
