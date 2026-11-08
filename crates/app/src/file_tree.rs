//! A committed tree's flat path list folded into folders, built once on the reader thread so the
//! Files pane only walks it. Git lists entries in its own sorted order, so each folder's files
//! and subfolders arrive already ordered and are kept as they come.

use repose_core::model::TreeEntry;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Folder {
	pub name: String,
	/// Slash-separated from the root; empty for the root itself.
	pub path: String,
	pub folders: Vec<Folder>,
	pub files: Vec<TreeEntry>,
}

impl Folder {
	/// Files in this folder and every folder below it.
	pub fn file_count(&self) -> usize {
		self.files.len() + self.folders.iter().map(Folder::file_count).sum::<usize>()
	}
}

pub fn build_folder_tree(entries: &[TreeEntry]) -> Folder {
	let mut root = Folder::default();
	for entry in entries {
		let parts: Vec<&str> = entry.path.split('/').collect();
		insert(&mut root, &parts, entry);
	}
	root
}

fn insert(folder: &mut Folder, parts: &[&str], entry: &TreeEntry) {
	match parts {
		[] => {}
		[_name] => folder.files.push(entry.clone()),
		[dir, rest @ ..] => {
			// Sorted input means the folder wanted is almost always the last one made.
			let found = folder
				.folders
				.last()
				.filter(|f| f.name == *dir)
				.map(|_| folder.folders.len() - 1)
				.or_else(|| folder.folders.iter().position(|f| f.name == *dir));
			let index = match found {
				Some(i) => i,
				None => {
					let path = if folder.path.is_empty() {
						(*dir).to_string()
					} else {
						format!("{}/{dir}", folder.path)
					};
					folder.folders.push(Folder {
						name: (*dir).to_string(),
						path,
						folders: Vec::new(),
						files: Vec::new(),
					});
					folder.folders.len() - 1
				}
			};
			insert(&mut folder.folders[index], rest, entry);
		}
	}
}

/// `512 B`, `1.2 KB`, `3.4 MB`, `4.0 GB`: each tier up to 1024 of the one below.
pub fn size_label(bytes: u64) -> String {
	const KB: f64 = 1024.0;
	if bytes < 1024 {
		format!("{bytes} B")
	} else if bytes < 1024 * 1024 {
		format!("{:.1} KB", bytes as f64 / KB)
	} else if bytes < 1024 * 1024 * 1024 {
		format!("{:.1} MB", bytes as f64 / (KB * KB))
	} else {
		format!("{:.1} GB", bytes as f64 / (KB * KB * KB))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn entry(path: &str) -> TreeEntry {
		TreeEntry {
			path: path.to_string(),
			size: 1,
			submodule: false,
		}
	}

	#[test]
	fn folds_paths_into_nested_folders_in_order() {
		let entries = [
			entry("README.md"),
			entry("crates/app/src/main.rs"),
			entry("crates/app/Cargo.toml"),
			entry("crates/core/src/lib.rs"),
			entry("docs/a.md"),
		];
		let root = build_folder_tree(&entries);
		assert_eq!(root.files.len(), 1);
		assert_eq!(root.file_count(), 5);
		let names: Vec<&str> = root.folders.iter().map(|f| f.name.as_str()).collect();
		assert_eq!(names, ["crates", "docs"]);
		let crates = &root.folders[0];
		assert_eq!(crates.path, "crates");
		assert_eq!(crates.folders.len(), 2);
		let app = &crates.folders[0];
		assert_eq!(app.path, "crates/app");
		assert_eq!(app.files[0].path, "crates/app/Cargo.toml");
		assert_eq!(app.folders[0].files[0].path, "crates/app/src/main.rs");
		assert_eq!(size_label(512), "512 B");
		assert_eq!(size_label(1536), "1.5 KB");
		assert_eq!(size_label(2 * 1_048_576), "2.0 MB");
		// A gigabyte tier, so 4 GiB is not "4096.0 MB" (audit-web-design-guidelines-1794001253).
		assert_eq!(size_label(1024 * 1024 * 1024 - 1), "1024.0 MB");
		assert_eq!(size_label(1024 * 1024 * 1024), "1.0 GB");
		assert_eq!(size_label(4 * 1024 * 1024 * 1024), "4.0 GB");
	}
}
