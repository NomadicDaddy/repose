//! The config file: roots to scan, names to skip, scan depth, and the external commands.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
	pub roots: Vec<PathBuf>,
	pub exclude: Vec<String>,
	pub depth: u32,
	/// `system` follows the Windows app theme, and `dark` or `light` overrides it.
	pub theme: Theme,
	pub commands: Commands,
}

/// The colour theme the window draws (light-theme clause 2). Any other value is a parse error
/// naming the file, as for every other key.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
	#[default]
	System,
	Dark,
	Light,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Commands {
	pub editor: String,
	pub terminal: String,
}

impl Default for Commands {
	fn default() -> Self {
		Self {
			editor: "code".into(),
			terminal: "wt".into(),
		}
	}
}

impl Default for Config {
	fn default() -> Self {
		Self {
			roots: [
				"D:\\applications",
				"D:\\applications.worktrees",
				"D:\\bbs",
				"D:\\amiga",
				"D:\\infra",
				"D:\\scripts",
			]
			.iter()
			.map(PathBuf::from)
			.collect(),
			exclude: ["node_modules", "target", "dist", ".prevers"]
				.iter()
				.map(|s| s.to_string())
				.collect(),
			depth: 3,
			theme: Theme::System,
			commands: Commands::default(),
		}
	}
}

#[derive(Debug)]
pub enum ConfigError {
	Io(PathBuf, std::io::Error),
	Parse(PathBuf, toml::de::Error),
	BadCommand(String),
}

impl std::fmt::Display for ConfigError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Io(p, e) => write!(f, "could not read {}: {e}", p.display()),
			Self::Parse(p, e) => write!(f, "could not parse {}: {e}", p.display()),
			Self::BadCommand(c) => {
				write!(f, "command must be a program name or an absolute path: {c}")
			}
		}
	}
}

impl std::error::Error for ConfigError {}

/// The default location: `%APPDATA%\repose\config.toml` on Windows, `~/.config/repose` elsewhere.
pub fn default_path() -> PathBuf {
	let base = std::env::var_os("APPDATA")
		.map(PathBuf::from)
		.or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
		.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
		.unwrap_or_else(|| PathBuf::from("."));
	base.join("repose").join("config.toml")
}

/// Loads the file, writing the defaults first when it does not exist.
pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
	if !path.exists() {
		let config = Config::default();
		if let Some(dir) = path.parent() {
			fs::create_dir_all(dir).map_err(|e| ConfigError::Io(path.to_path_buf(), e))?;
		}
		let text = toml::to_string_pretty(&config).unwrap_or_default();
		fs::write(path, text).map_err(|e| ConfigError::Io(path.to_path_buf(), e))?;
		return Ok(config);
	}
	let text = fs::read_to_string(path).map_err(|e| ConfigError::Io(path.to_path_buf(), e))?;
	let config: Config =
		toml::from_str(&text).map_err(|e| ConfigError::Parse(path.to_path_buf(), e))?;
	validate(&config)?;
	Ok(config)
}

/// A command that is not a program name or absolute path is an error with the offending value
/// named, never a panic. A missing root is not an error: the app starts and reports it, because
/// one unplugged drive should not keep the other roots from showing.
pub fn validate(config: &Config) -> Result<(), ConfigError> {
	for command in [&config.commands.editor, &config.commands.terminal] {
		let plain = !command.contains([' ', '"', '\'', '|', '&', ';']);
		let shape = Path::new(command).is_absolute() || !command.contains(['/', '\\']);
		if command.is_empty() || !plain || !shape {
			return Err(ConfigError::BadCommand(command.clone()));
		}
	}
	Ok(())
}

/// Whether a folder name is on the config's exclude list. Windows names folders without regard
/// to ASCII case, so the walk and the watcher both compare this way (audit data-architecture).
pub fn name_excluded(name: &str, exclude: &[String]) -> bool {
	exclude.iter().any(|e| e.eq_ignore_ascii_case(name))
}

/// The configured roots that are not directories right now, for the status line to name.
pub fn missing_roots(config: &Config) -> Vec<PathBuf> {
	config
		.roots
		.iter()
		.filter(|r| !r.is_dir())
		.cloned()
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn defaults_round_trip() {
		let text = toml::to_string_pretty(&Config::default()).unwrap_or_default();
		let back: Config = toml::from_str(&text).ok().unwrap_or_default();
		assert_eq!(back, Config::default());
	}

	#[test]
	fn the_theme_key_takes_system_dark_or_light_and_refuses_anything_else() {
		let read = |text: &str| toml::from_str::<Config>(text).map(|c| c.theme);
		assert_eq!(read("").ok(), Some(Theme::System), "missing is system");
		assert_eq!(read("theme = \"system\"").ok(), Some(Theme::System));
		assert_eq!(read("theme = \"dark\"").ok(), Some(Theme::Dark));
		assert_eq!(read("theme = \"light\"").ok(), Some(Theme::Light));
		assert!(read("theme = \"Light\"").is_err(), "values are lowercase");
		assert!(read("theme = \"sepia\"").is_err());
	}

	#[test]
	fn rejects_a_shell_command() {
		let mut config = Config {
			roots: Vec::new(),
			..Config::default()
		};
		config.commands.editor = "code --wait".into();
		assert!(matches!(validate(&config), Err(ConfigError::BadCommand(_))));
	}

	#[test]
	fn a_missing_root_is_reported_not_fatal() {
		let config = Config {
			roots: vec![PathBuf::from("Q:\\no\\such\\root\\for\\repose")],
			..Config::default()
		};
		assert!(validate(&config).is_ok());
		assert_eq!(missing_roots(&config), config.roots);
	}
}
