//! The settings screen's save (settings-screen clause 3): the roots, excludes, depth, theme and
//! commands written back into config.toml in place. Only a key whose value changed is rewritten,
//! and it keeps the spacing and the comment on its own line; every other key, comment and line
//! stays as it was. `pinned_repositories` and any key repose does not know are never touched.
//! Nothing here runs git.

use std::fs;
use std::path::Path;

use toml_edit::{DocumentMut, Item, Table, Value};

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
		set(&mut doc, "roots", path_array(&next.roots).into());
	}
	if next.exclude != before.exclude {
		let names: toml_edit::Array = next.exclude.iter().map(String::as_str).collect();
		set(&mut doc, "exclude", names.into());
	}
	if next.depth != before.depth {
		set(&mut doc, "depth", i64::from(next.depth).into());
	}
	if next.theme != before.theme {
		set(&mut doc, "theme", theme_name(next.theme).into());
	}
	if next.commands != before.commands {
		let commands = commands_table(&mut doc, path)?;
		if next.commands.editor != before.commands.editor {
			set(commands, "editor", next.commands.editor.as_str().into());
		}
		if next.commands.terminal != before.commands.terminal {
			set(commands, "terminal", next.commands.terminal.as_str().into());
		}
	}
	config::replace(path, &doc.to_string())
}

/// Sets `key` in `table` to `new`. When the key already holds a value, that value's decoration
/// (the spacing around it and a comment after it on the same line) is kept, so a changed line
/// keeps its comment (Roger S1, Morticia 5Q-4 on 25e8094).
fn set(table: &mut Table, key: &str, mut new: Value) {
	if let Some(old) = table.get(key).and_then(Item::as_value) {
		*new.decor_mut() = old.decor().clone();
	}
	// Through the index, not `insert`: the existing key, with the comment lines above it, stays.
	table[key] = Item::Value(new);
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
