//! The breadcrumb shared by the diff pane and the file viewer: `repository › folder › file`,
//! the repository name a link back to its header.

use std::path::Path;

use eframe::egui::{FontFamily, RichText, Ui};

use crate::theme::*;

/// Draws the path crumbs into the current horizontal row and returns true when the repository
/// link was clicked.
pub(super) fn path(ui: &mut Ui, repo: &Path, display: &str) -> bool {
	let repo_name = repo
		.file_name()
		.map(|n| n.to_string_lossy().to_string())
		.unwrap_or_default();
	let (folder, file) = match display.rsplit_once('/') {
		Some((f, n)) => (f, n),
		None => ("", display),
	};
	let go_repo = ui
		.link(
			RichText::new(repo_name)
				.family(FontFamily::Monospace)
				.color(ACCENT),
		)
		.clicked();
	if !folder.is_empty() {
		sep(ui);
		ui.label(RichText::new(folder).color(MUTED));
	}
	sep(ui);
	ui.label(RichText::new(file).color(TEXT_BRIGHT).strong());
	go_repo
}

fn sep(ui: &mut Ui) {
	ui.label(RichText::new("›").color(MUTED));
}
