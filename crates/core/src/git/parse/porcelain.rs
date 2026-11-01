//! `git status --porcelain=v2 -z --branch`: headers, ordinary entries, renames (two NUL-separated
//! paths), unmerged entries, untracked and ignored.

use crate::model::{BranchState, StatusSummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
	/// `1`: an ordinary change; `xy` is the two-letter index/worktree code.
	Changed { xy: [u8; 2] },
	/// `2`: a rename or copy; `score` is like `R100`.
	Renamed {
		xy: [u8; 2],
		score: String,
		from: String,
	},
	/// `u`: unmerged.
	Unmerged { xy: [u8; 2] },
	/// `?`
	Untracked,
	/// `!`
	Ignored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
	pub kind: EntryKind,
	pub path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Porcelain {
	pub oid: Option<String>,
	pub head: Option<String>,
	pub upstream: Option<String>,
	pub ahead: Option<u32>,
	pub behind: Option<u32>,
	pub entries: Vec<Entry>,
}

impl Porcelain {
	pub fn branch_state(&self) -> BranchState {
		match (&self.head, &self.oid) {
			(_, Some(oid)) if oid == "(initial)" => BranchState::Unborn,
			(Some(h), _) if h == "(detached)" => BranchState::Detached,
			(Some(h), _) => BranchState::Branch(h.clone()),
			(None, _) => BranchState::Unborn,
		}
	}

	pub fn short_hash(&self) -> Option<String> {
		self.oid
			.as_ref()
			.filter(|o| *o != "(initial)")
			.map(|o| o.chars().take(7).collect())
	}

	pub fn summary(&self) -> StatusSummary {
		let mut s = StatusSummary {
			upstream: self.upstream.clone(),
			ahead: self.ahead,
			behind: self.behind,
			..StatusSummary::default()
		};
		for e in &self.entries {
			match &e.kind {
				EntryKind::Changed { xy } | EntryKind::Renamed { xy, .. } => {
					if xy[0] != b'.' {
						s.staged += 1;
					}
					if xy[1] != b'.' {
						s.unstaged += 1;
					}
				}
				EntryKind::Unmerged { .. } => s.conflicts += 1,
				EntryKind::Untracked => s.untracked += 1,
				EntryKind::Ignored => {}
			}
		}
		s
	}
}

/// Parses the NUL-separated records. Unknown record types are skipped, never fatal.
pub fn parse_porcelain_v2(bytes: &[u8]) -> Porcelain {
	let mut out = Porcelain::default();
	let mut records = bytes.split(|b| *b == 0).filter(|r| !r.is_empty());
	while let Some(record) = records.next() {
		let text = String::from_utf8_lossy(record).to_string();
		if let Some(header) = text.strip_prefix("# ") {
			header_line(header, &mut out);
			continue;
		}
		let mut fields = text.splitn(2, ' ');
		let tag = fields.next().unwrap_or("");
		let rest = fields.next().unwrap_or("");
		match tag {
			"1" => {
				if let Some((xy, path)) = ordinary(rest) {
					out.entries.push(Entry {
						kind: EntryKind::Changed { xy },
						path,
					});
				}
			}
			"2" => {
				let from = records
					.next()
					.map(|r| String::from_utf8_lossy(r).to_string())
					.unwrap_or_default();
				if let Some((xy, score, path)) = rename(rest) {
					out.entries.push(Entry {
						kind: EntryKind::Renamed { xy, score, from },
						path,
					});
				}
			}
			"u" => {
				if let Some((xy, path)) = unmerged(rest) {
					out.entries.push(Entry {
						kind: EntryKind::Unmerged { xy },
						path,
					});
				}
			}
			"?" => out.entries.push(Entry {
				kind: EntryKind::Untracked,
				path: rest.to_string(),
			}),
			"!" => out.entries.push(Entry {
				kind: EntryKind::Ignored,
				path: rest.to_string(),
			}),
			_ => {}
		}
	}
	out
}

fn header_line(header: &str, out: &mut Porcelain) {
	let Some((key, value)) = header.split_once(' ') else {
		return;
	};
	match key {
		"branch.oid" => out.oid = Some(value.to_string()),
		"branch.head" => out.head = Some(value.to_string()),
		"branch.upstream" => out.upstream = Some(value.to_string()),
		"branch.ab" => {
			let mut parts = value.split(' ');
			out.ahead = parts
				.next()
				.and_then(|a| a.trim_start_matches('+').parse().ok());
			out.behind = parts
				.next()
				.and_then(|b| b.trim_start_matches('-').parse().ok());
		}
		_ => {}
	}
}

fn xy_of(s: &str) -> Option<[u8; 2]> {
	let b = s.as_bytes();
	(b.len() == 2).then(|| [b[0], b[1]])
}

/// `<XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>`
fn ordinary(rest: &str) -> Option<([u8; 2], String)> {
	let mut parts = rest.splitn(8, ' ');
	let xy = xy_of(parts.next()?)?;
	for _ in 0..6 {
		parts.next()?;
	}
	Some((xy, parts.next()?.to_string()))
}

/// `<XY> <sub> <mH> <mI> <mW> <hH> <hI> <Xscore> <path>`; the origin follows as its own record.
fn rename(rest: &str) -> Option<([u8; 2], String, String)> {
	let mut parts = rest.splitn(9, ' ');
	let xy = xy_of(parts.next()?)?;
	for _ in 0..6 {
		parts.next()?;
	}
	let score = parts.next()?.to_string();
	Some((xy, score, parts.next()?.to_string()))
}

/// `<XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>`
fn unmerged(rest: &str) -> Option<([u8; 2], String)> {
	let mut parts = rest.splitn(10, ' ');
	let xy = xy_of(parts.next()?)?;
	for _ in 0..8 {
		parts.next()?;
	}
	Some((xy, parts.next()?.to_string()))
}
