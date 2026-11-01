//! Syntax colour for diff lines: syntect with the fancy-regex engine and two-face's extra
//! syntaxes (TypeScript, TOML, PowerShell and more), a dark base16 theme, chosen by file
//! extension. An unknown extension gets no spans and draws as plain text. Computed on the reader
//! thread alongside the rows, never while drawing.

use std::ops::Range;
use std::sync::OnceLock;

use eframe::egui::Color32;
use syntect::easy::HighlightLines;
use syntect::highlighting::Theme;
use syntect::parsing::{SyntaxReference, SyntaxSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
	pub range: Range<usize>,
	pub color: Color32,
}

struct Engine {
	syntaxes: SyntaxSet,
	theme: Theme,
}

fn engine() -> &'static Engine {
	static ENGINE: OnceLock<Engine> = OnceLock::new();
	ENGINE.get_or_init(|| Engine {
		syntaxes: two_face::syntax::extra_newlines(),
		theme: two_face::theme::extra()
			.get(two_face::theme::EmbeddedThemeName::Base16OceanDark)
			.clone(),
	})
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

/// Colour spans for each line of one file, in the order given. State carries from line to line,
/// so a block comment that opens on one line stays a comment on the next; it does not carry
/// across hunks, which is the usual compromise for a diff.
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
			let Ok(ranges) = h.highlight_line(line, &e.syntaxes) else {
				return Vec::new();
			};
			let mut spans = Vec::new();
			let mut pos = 0usize;
			for (style, text) in ranges {
				let end = pos + text.len();
				if Some(style.foreground) != plain {
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
		// two-face 0.4.5 ships no PowerShell syntax; a .ps1 draws as plain text until one is
		// added (diff-viewer record, clause 5). This assertion is the reminder.
		assert!(syntax_for("scripts/qc.ps1").is_none());
		assert!(colour_lines("scripts/qc.ps1", &["param([string] $Exe)"])[0].is_empty());
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
}
