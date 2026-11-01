//! Bounded directory walk: every `.git` directory or file to the configured depth, with the
//! excluded names pruned.

use std::fs;
use std::path::Path;

use super::classify::{Found, classify};
use super::skipped;

pub fn walk(root: &Path, depth: u32, exclude: &[String], out: &mut Vec<Found>) {
	if !root.is_dir() {
		return;
	}
	visit(root, depth, exclude, out);
}

fn visit(dir: &Path, depth: u32, exclude: &[String], out: &mut Vec<Found>) {
	if let Some(found) = classify(dir) {
		out.push(found);
	}
	if depth == 0 {
		return;
	}
	let Ok(entries) = fs::read_dir(dir) else {
		return;
	};
	let mut children: Vec<_> = entries
		.flatten()
		.filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
		.map(|e| e.path())
		.collect();
	children.sort();
	for child in children {
		let name = child
			.file_name()
			.map(|n| n.to_string_lossy().to_string())
			.unwrap_or_default();
		if skipped(&name, exclude) {
			continue;
		}
		visit(&child, depth - 1, exclude, out);
	}
}
