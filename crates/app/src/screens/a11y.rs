//! What assistive technology reads for the painted screens (accessible-rendered-content). egui
//! gives a node to a widget only when it reports `WidgetInfo`, and a painted row reports nothing
//! of its own, so each clickable row names itself here through its response, with the role egui
//! uses for its own selectable labels (a button with a selected state), and the read-only text
//! the screens paint (diff lines, separators, group counts) gets a plain node through
//! `Context::accesskit_node_builder`: in the tree, but never focusable, so the diff adds no Tab
//! stops. Tooltips are left as they are.

use eframe::egui::{Id, Rect, Response, Role, Ui, WidgetInfo, accesskit};
use repose_core::model::{Branch, ChangeKind, Commit, DiffLine, FileChange, LineKind, RepoSummary};

/// A painted control: its role, its name and, for a row that can be selected, open or current,
/// whether it is.
pub(super) fn control(response: &Response, role: Role, selected: Option<bool>, name: &str) {
	response.widget_info(|| match selected {
		Some(s) => WidgetInfo::selected(role, true, s, name),
		None => WidgetInfo::labeled(role, true, name),
	});
}

/// A label whose text reports progress or an outcome (scanning, reading status, a commit's
/// result): its node becomes a polite live region named by the text, so assistive technology
/// announces a change without the user moving to it (audit-web-design-guidelines-1793709090).
pub(super) fn live(response: &Response, text: &str) {
	response.ctx.accesskit_node_builder(response.id, |node| {
		node.set_role(Role::Status);
		node.set_live(accesskit::Live::Polite);
		node.set_label(text);
	});
}

/// Read-only painted text: a node with the text and where it sits, and no actions.
pub(super) fn text(ui: &Ui, id: Id, rect: Rect, role: Role, text: &str) {
	ui.ctx().accesskit_node_builder(id, |node| {
		node.set_role(role);
		node.set_bounds(accesskit::Rect {
			x0: rect.min.x.into(),
			y0: rect.min.y.into(),
			x1: rect.max.x.into(),
			y1: rect.max.y.into(),
		});
		if role == Role::Label {
			node.set_value(text);
		} else {
			node.set_label(text);
		}
	});
}

/// A repository or worktree row: name, branch and commit, path, then the counts its pills show,
/// in words rather than arrows.
pub(super) fn repo_name(repo: &RepoSummary) -> String {
	let mut parts = vec![
		repo.name.clone(),
		repo.branch_line().replace(" · ", " at "),
		repo.path.display().to_string(),
	];
	if repo.error.is_some() {
		parts.push("error".into());
	} else if let Some(s) = &repo.status {
		if let Some(b) = s.behind.filter(|b| *b > 0) {
			parts.push(format!("{b} behind"));
		}
		if let Some(a) = s.ahead.filter(|a| *a > 0) {
			parts.push(format!("{a} ahead"));
		}
		if s.conflicts > 0 {
			parts.push(format!("{} conflicts", s.conflicts));
		}
		let changed = s.changed() - s.conflicts;
		if changed > 0 {
			parts.push(format!("{changed} changed"));
		}
	}
	parts.join(", ")
}

/// The worktree chevron, named by what a click does.
pub(super) fn chevron_name(expanded: bool, count: usize) -> String {
	let noun = if count == 1 { "worktree" } else { "worktrees" };
	let verb = if expanded { "Collapse" } else { "Expand" };
	format!("{verb} {count} {noun}")
}

/// A branch row: name, current, upstream and the saved counts.
pub(super) fn branch_name(branch: &Branch) -> String {
	let mut parts = vec![branch.name.clone(), format!("at {}", branch.short_hash)];
	if branch.current {
		parts.push("current".into());
	}
	// "Upstream gone" exactly when the painted row says it: an upstream with neither count
	// (branches.rs; Morticia and Roger Info on 8eebf0e).
	match (&branch.upstream, branch.ahead, branch.behind) {
		(Some(u), None, None) => parts.push(format!("tracks {u}, upstream gone")),
		(Some(u), ahead, behind) => {
			parts.push(format!("tracks {u}"));
			if let Some(a) = ahead {
				parts.push(format!("{a} ahead"));
			}
			if let Some(b) = behind {
				parts.push(format!("{b} behind"));
			}
		}
		(None, _, _) => parts.push("no upstream".into()),
	}
	parts.join(", ")
}

/// A commit row: short hash and subject, then author and when, as the row shows them.
pub(super) fn commit_name(c: &Commit, when: &str) -> String {
	format!("{} {}, {}, {when}", c.short, c.subject, c.author)
}

/// A change row: the path, what happened to it and, for a rename, where it came from.
pub(super) fn change_name(file: &FileChange) -> String {
	let kind = match file.kind {
		ChangeKind::Modified => "modified",
		ChangeKind::Added => "added",
		ChangeKind::Deleted => "deleted",
		ChangeKind::Renamed => "renamed",
		ChangeKind::Untracked => "untracked",
		ChangeKind::Conflicted => "conflicted",
		ChangeKind::TypeChanged => "type changed",
		ChangeKind::IntentToAdd => "intent to add",
	};
	match &file.from {
		Some(from) => format!("{}, {kind} from {from}", file.path),
		None => format!("{}, {kind}", file.path),
	}
}

/// One diff line with its numbers and kind, as the gutters and colours show it.
pub(super) fn diff_line(l: &DiffLine) -> String {
	let n = |no: Option<u32>| no.map_or_else(String::new, |n| n.to_string());
	match l.kind {
		LineKind::Added => format!("added line {}: {}", n(l.new_no), l.text),
		LineKind::Removed => format!("removed line {}: {}", n(l.old_no), l.text),
		LineKind::Context => format!("line {}: {}", n(l.new_no), l.text),
	}
}

/// A separator between hunks, as painted.
pub(super) fn separator(skipped: u32, heading: &str) -> String {
	if heading.is_empty() {
		format!("{skipped} unchanged lines")
	} else {
		format!("{skipped} unchanged lines, {heading}")
	}
}
