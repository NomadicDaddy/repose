//! Syntax colour for diff and file lines: syntect with the fancy-regex engine, two-face's extra
//! syntaxes (TypeScript, TOML and more) plus the vendored PowerShell grammar under
//! `crates/app/syntaxes/PowerShell` (two-face 0.4.5 has none; see SOURCE.md there), a dark base16
//! theme, chosen by file extension. An unknown extension gets no spans and draws as plain text.
//! Computed on the reader thread alongside the rows, never while drawing; lines over
//! MAX_COLOUR_BYTES are left plain.

use std::ops::Range;
use std::sync::OnceLock;

use eframe::egui::Color32;
use syntect::easy::HighlightLines;
use syntect::highlighting::Theme;
use syntect::parsing::{SyntaxDefinition, SyntaxReference, SyntaxSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
	pub range: Range<usize>,
	pub color: Color32,
}

struct Engine {
	syntaxes: SyntaxSet,
	theme: Theme,
}

/// The MIT grammar from SublimeText/PowerShell; provenance and the one local edit in SOURCE.md.
const POWERSHELL_SYNTAX: &str = include_str!("../syntaxes/PowerShell/PowerShell.sublime-syntax");

fn engine() -> &'static Engine {
	static ENGINE: OnceLock<Engine> = OnceLock::new();
	ENGINE.get_or_init(|| Engine {
		syntaxes: syntaxes(),
		theme: two_face::theme::extra()
			.get(two_face::theme::EmbeddedThemeName::Base16OceanDark)
			.clone(),
	})
}

/// two-face's set with PowerShell added and the references linked again. The grammar is a file
/// in this repository, so a failure to parse it is a build-time mistake, not a user's; the test
/// below asserts it loads, and at run time the colouring falls back to two-face's set alone.
fn syntaxes() -> SyntaxSet {
	let base = two_face::syntax::extra_newlines();
	match SyntaxDefinition::load_from_str(POWERSHELL_SYNTAX, true, None) {
		Ok(powershell) => {
			let mut builder = base.into_builder();
			builder.add(powershell);
			builder.build()
		}
		Err(_) => base,
	}
}

/// The syntax for a path, by its extension, with the names syntect uses for the few that do not
/// match their extension.
fn syntax_for(path: &str) -> Option<&'static SyntaxReference> {
	let e = engine();
	let name = path.rsplit('/').next().unwrap_or(path);
	let ext = name.rsplit_once('.').map(|(_, x)| x.to_ascii_lowercase());
	let ext = ext.as_deref()?;
	let by_name = match ext {
		"ts" | "mts" | "cts" => Some("TypeScript"),
		"tsx" => Some("TypeScriptReact"),
		"ps1" | "psm1" | "psd1" => Some("PowerShell"),
		"cs" => Some("C#"),
		"sh" | "bash" | "zsh" => Some("Bourne Again Shell (bash)"),
		_ => None,
	};
	by_name
		.and_then(|n| e.syntaxes.find_syntax_by_name(n))
		.or_else(|| e.syntaxes.find_syntax_by_extension(ext))
}

/// Lines longer than this are left plain: a minified bundle took seconds to colour (Morticia on
/// e8e10ad), and colour on a line nobody can read buys nothing.
pub const MAX_COLOUR_BYTES: usize = 4 * 1024;

/// Colour spans for each line of one file, in the order given. State carries from line to line,
/// so a block comment that opens on one line stays a comment on the next; it does not carry
/// across hunks, which is the usual compromise for a diff. The `_newlines` syntaxes expect each
/// line to end in a newline (a `#` comment would otherwise swallow the rest of the hunk, Morticia
/// M-1 on e8e10ad), so one is supplied and the ranges are clipped back to the text.
pub fn colour_lines(path: &str, lines: &[&str]) -> Vec<Vec<Span>> {
	let Some(syntax) = syntax_for(path) else {
		return lines.iter().map(|_| Vec::new()).collect();
	};
	let e = engine();
	let mut h = HighlightLines::new(syntax, &e.theme);
	let plain = e.theme.settings.foreground;
	lines
		.iter()
		.map(|line| {
			if line.len() > MAX_COLOUR_BYTES {
				// The parser never sees this line, so whatever it opened or closed is unknown;
				// start the lines after it from a clean state rather than the one before it.
				h = HighlightLines::new(syntax, &e.theme);
				return Vec::new();
			}
			let with_newline = format!("{line}\n");
			let Ok(ranges) = h.highlight_line(&with_newline, &e.syntaxes) else {
				return Vec::new();
			};
			let mut spans = Vec::new();
			let mut pos = 0usize;
			for (style, text) in ranges {
				let end = (pos + text.len()).min(line.len());
				if end > pos && Some(style.foreground) != plain {
					let c = style.foreground;
					spans.push(Span {
						range: pos..end,
						color: Color32::from_rgb(c.r, c.g, c.b),
					});
				}
				pos = end;
			}
			spans
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn known_extensions_colour_and_unknown_ones_do_not() {
		let ts = colour_lines("src/a.ts", &["const x: number = 1;"]);
		assert!(!ts[0].is_empty(), "TypeScript gets spans");
		let rs = colour_lines("lib.rs", &["fn main() {}", "// note"]);
		assert_eq!(rs.len(), 2);
		assert!(!rs[0].is_empty());
		for (path, line) in [
			("a.json", "{\"k\": 1}"),
			("a.md", "# Title"),
			("a.yaml", "key: value"),
			("a.css", "a { color: red; }"),
			("a.html", "<div class=\"x\"></div>"),
			("a.py", "def f(): return 1"),
			("a.cs", "public class A {}"),
			("a.sh", "echo \"$HOME\""),
			("a.js", "const x = 1;"),
		] {
			assert!(
				!colour_lines(path, &[line])[0].is_empty(),
				"{path} gets spans"
			);
		}
		let toml = colour_lines("Cargo.toml", &["[package]", "name = \"repose\""]);
		assert!(
			!toml[0].is_empty() || !toml[1].is_empty(),
			"TOML: syntax {:?}, spans {toml:?}",
			syntax_for("Cargo.toml").map(|s| s.name.clone())
		);
		let none = colour_lines("notes.weirdext", &["anything at all"]);
		assert!(
			none[0].is_empty(),
			"unknown extension: plain text, no error"
		);
		let bare = colour_lines("LICENSE", &["text"]);
		assert!(bare[0].is_empty());
	}

	/// Diff-viewer clause 5, Phillip #30926: the vendored grammar loads, the three PowerShell
	/// extensions colour, a here-string that embeds C# (the one edited reference) still colours
	/// the lines after it, and building the set stays well under a second.
	#[test]
	fn powershell_is_coloured_from_the_vendored_grammar() {
		let started = std::time::Instant::now();
		let set = syntaxes();
		let built = started.elapsed();
		assert!(
			set.find_syntax_by_name("PowerShell").is_some(),
			"the vendored grammar did not load"
		);
		assert!(
			built < std::time::Duration::from_secs(3),
			"set built in {built:?}"
		);
		for path in ["scripts/qc.ps1", "Module.psm1", "Manifest.psd1"] {
			assert_eq!(
				syntax_for(path).map(|s| s.name.as_str()),
				Some("PowerShell"),
				"{path}"
			);
			let out = colour_lines(
				path,
				&["param([string] $Exe)", "# note", "Write-Host \"hi $Exe\""],
			);
			assert!(out.iter().all(|l| !l.is_empty()), "{path}: {out:?}");
		}
		let embedded = colour_lines(
			"a.ps1",
			&[
				"$src = @\"",
				"using System;",
				"public static class W { public static int F() { return 1; } }",
				"\"@",
				"Add-Type -TypeDefinition $src",
				"$x = 1",
			],
		);
		assert_eq!(embedded.len(), 6);
		assert!(
			!embedded[5].is_empty(),
			"colouring resumes after the here-string"
		);
	}

	/// Morticia M-1 on e8e10ad: without the newline the `#` comment never closed and the next
	/// line came back as one comment-coloured run.
	#[test]
	fn a_hash_comment_ends_at_its_line() {
		for (path, comment, code) in [
			("a.py", "# note", "def f(): pass"),
			("a.sh", "# note", "echo hi"),
		] {
			let out = colour_lines(path, &[comment, code]);
			assert!(!out[0].is_empty(), "{path}: the comment line is coloured");
			let comment_colour = out[0].first().map_or(Color32::TRANSPARENT, |s| s.color);
			assert!(
				out[0].iter().all(|s| s.range.end <= comment.len()),
				"ranges clipped to the text: {:?}",
				out[0]
			);
			let next_all_comment = !out[1].is_empty()
				&& out[1]
					.iter()
					.all(|s| s.color == comment_colour && s.range == (0..code.len()));
			assert!(!next_all_comment, "{path}: {:?}", out[1]);
			assert!(!out[1].is_empty(), "{path}: the code line is coloured");
		}
		let long = "x".repeat(MAX_COLOUR_BYTES + 1);
		assert!(colour_lines("a.py", &[&long])[0].is_empty());
		// Roger on bed5ab8: a skipped line that opens a string must not leave the parser inside
		// it; the line after a skipped line is coloured as if it started the file.
		let opener = format!("s = \"\"\"{}", "y".repeat(MAX_COLOUR_BYTES));
		let after = colour_lines("a.py", &[&opener, "def f(): pass"]);
		let alone = colour_lines("a.py", &["def f(): pass"]);
		assert!(after[0].is_empty());
		assert_eq!(after[1], alone[0]);
	}
}
