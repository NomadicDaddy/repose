//! The settings screen's save (settings-screen clause 3): the roots, excludes, depth, theme and
//! commands written back into config.toml in place. Only a key whose value changed is rewritten,
//! so every other key, every comment and the layout of the keys left alone stay as they were;
//! `pinned_repositories` and any key repose does not know are never touched. Nothing here runs
//! git.

use std::fs;
use std::path::Path;

use toml_edit::{DocumentMut, Item, Table, value};

use crate::config::{self, Config, ConfigError, Theme};
use crate::pins::path_array;

/// Writes the settings of `next` that differ from `before` into the config file at `path`. The
/// file is replaced in one rename, so a failed write leaves it whole; the error names the file.
/// `next` is validated first: a command that is not a program name or absolute path is refused
/// before anything is written.
pub fn save(path: &Path, before: &Config, next: &Config) -> Result<(), ConfigError> {
	config::validate(next)?;
	let text = fs::read_to_string(path).map_err(|e| ConfigError::Io(path.to_path_buf(), e))?;
	let mut doc: DocumentMut = text
		.parse()
		.map_err(|e: toml_edit::TomlError| ConfigError::Edit(path.to_path_buf(), e.to_string()))?;
	if next.roots != before.roots {
		doc["roots"] = value(path_array(&next.roots));
	}
	if next.exclude != before.exclude {
		let names: toml_edit::Array = next.exclude.iter().map(String::as_str).collect();
		doc["exclude"] = value(names);
	}
	if next.depth != before.depth {
		doc["depth"] = value(i64::from(next.depth));
	}
	if next.theme != before.theme {
		doc["theme"] = value(theme_name(next.theme));
	}
	if next.commands != before.commands {
		let commands = commands_table(&mut doc, path)?;
		if next.commands.editor != before.commands.editor {
			commands["editor"] = value(next.commands.editor.as_str());
		}
		if next.commands.terminal != before.commands.terminal {
			commands["terminal"] = value(next.commands.terminal.as_str());
		}
	}
	config::replace(path, &doc.to_string())
}

/// The value config.toml spells a theme with, as serde reads it back.
fn theme_name(theme: Theme) -> &'static str {
	match theme {
		Theme::System => "system",
		Theme::Dark => "dark",
		Theme::Light => "light",
	}
}

/// The file's `[commands]` table, created when the file has none. A `commands` key that is not a
/// table (an inline table included) is refused rather than rewritten in another shape.
fn commands_table<'a>(doc: &'a mut DocumentMut, path: &Path) -> Result<&'a mut Table, ConfigError> {
	let entry = doc.entry("commands").or_insert(Item::Table(Table::new()));
	entry.as_table_mut().ok_or_else(|| {
		ConfigError::Edit(
			path.to_path_buf(),
			"commands is not a [commands] table".into(),
		)
	})
}
