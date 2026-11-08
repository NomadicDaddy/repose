//! The lane layout behind History's optional branch graph (branch-graph-lanes). Commits come
//! newest first in topological order, so every child is laid out before its parents. A lane is a
//! column waiting for one commit: a commit takes the lane that waits for it (or a free one when
//! it is a branch tip), and hands the lane on to its first parent; each further parent waits in
//! a lane of its own, which is where a merge fans out and, further down, where the lanes join
//! again at the fork. Columns never move, so a straight lane stays straight.

use repose_core::model::Commit;

/// How one row's graph is drawn: the commit's own column, and the line segments in the row's
/// upper half (from a column at the top edge to a column at the middle) and lower half (from a
/// column at the middle to a column at the bottom edge). A segment whose ends differ is a
/// connector: a lane merging into the commit above it, or the commit forking a lane below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphRow {
	pub node: usize,
	pub up: Vec<(usize, usize)>,
	pub down: Vec<(usize, usize)>,
	/// Columns this row spans, for the width the graph needs.
	pub width: usize,
}

/// The rows for `commits` in the order given.
pub fn layout(commits: &[Commit]) -> Vec<GraphRow> {
	let mut lanes: Vec<Option<&str>> = Vec::new();
	let mut rows = Vec::with_capacity(commits.len());
	for c in commits {
		let hash = c.hash.as_str();
		let node = match lanes.iter().position(|l| *l == Some(hash)) {
			Some(i) => i,
			None => free(&mut lanes),
		};
		let mut up = Vec::new();
		for (i, lane) in lanes.iter().enumerate() {
			match lane {
				Some(h) if *h == hash => up.push((i, node)),
				Some(_) => up.push((i, i)),
				None => {}
			}
		}
		// Every lane that waited for this commit ends at it; the ones from the side are merges.
		for lane in lanes.iter_mut() {
			if *lane == Some(hash) {
				*lane = None;
			}
		}
		let width_before = lanes.len().max(node + 1);
		let mut down = Vec::new();
		let mut started = Vec::new();
		for (n, parent) in c.parents.iter().enumerate() {
			let target = match lanes.iter().position(|l| *l == Some(parent.as_str())) {
				Some(i) => i,
				None => {
					let i = if n == 0 { node } else { free(&mut lanes) };
					lanes[i] = Some(parent.as_str());
					started.push(i);
					i
				}
			};
			down.push((node, target));
		}
		for (i, lane) in lanes.iter().enumerate() {
			if lane.is_some() && !started.contains(&i) {
				down.push((i, i));
			}
		}
		while lanes.last() == Some(&None) {
			lanes.pop();
		}
		rows.push(GraphRow {
			node,
			up,
			down,
			width: width_before.max(lanes.len()),
		});
	}
	rows
}

/// The first free column, or a new one on the right.
fn free(lanes: &mut Vec<Option<&str>>) -> usize {
	match lanes.iter().position(Option::is_none) {
		Some(i) => i,
		None => {
			lanes.push(None);
			lanes.len() - 1
		}
	}
}

#[cfg(test)]
mod tests {
	use repose_core::model::Commit;

	use super::{GraphRow, layout};

	fn commit(hash: &str, parents: &[&str]) -> Commit {
		Commit {
			hash: hash.to_string(),
			parents: parents.iter().map(|p| p.to_string()).collect(),
			..Commit::default()
		}
	}

	/// Clause 3: a repository with only main is one straight lane.
	#[test]
	fn only_main_is_one_straight_lane() {
		let rows = layout(&[commit("c", &["b"]), commit("b", &["a"]), commit("a", &[])]);
		for (i, row) in rows.iter().enumerate() {
			assert_eq!(row.node, 0, "row {i}");
			assert_eq!(row.width, 1, "row {i}");
		}
		assert_eq!(rows[0].up, vec![], "the tip has nothing above it");
		assert_eq!(rows[0].down, vec![(0, 0)]);
		assert_eq!(rows[1].up, vec![(0, 0)]);
		assert_eq!(rows[2].down, vec![], "the root has nothing below it");
	}

	/// Clause 3: two branches and one merge draw two lanes, which fan out at the merge commit
	/// and join again at the commit the side branch forked from.
	#[test]
	fn a_merge_draws_two_lanes_that_meet_at_the_merge_and_the_fork() {
		// m merges s into main; main moved on with a; both forked from b.
		let rows = layout(&[
			commit("m", &["a", "s"]),
			commit("a", &["b"]),
			commit("s", &["b"]),
			commit("b", &[]),
		]);
		let m = &rows[0];
		assert_eq!((m.node, m.width), (0, 2));
		assert_eq!(
			m.down,
			vec![(0, 0), (0, 1)],
			"the merge fans out into both lanes"
		);
		let a = &rows[1];
		assert_eq!(a.node, 0);
		assert_eq!(a.up, vec![(0, 0), (1, 1)], "the side lane passes a by");
		let s = &rows[2];
		assert_eq!(s.node, 1, "the side commit sits in the second lane");
		assert_eq!(
			s.down,
			vec![(1, 0), (0, 0)],
			"and joins main's lane at the fork"
		);
		assert_eq!(
			rows[3],
			GraphRow {
				node: 0,
				up: vec![(0, 0)],
				down: vec![],
				width: 1
			},
			"the fork point is back to one lane"
		);
	}

	/// A branch tip that nothing above waits for starts its own lane, and an unrelated lane
	/// beside it stays straight.
	#[test]
	fn a_second_tip_takes_a_free_lane_and_others_stay_straight() {
		let rows = layout(&[
			commit("t", &["x"]),
			commit("u", &["y"]),
			commit("x", &[]),
			commit("y", &[]),
		]);
		assert_eq!(rows[1].node, 1);
		assert_eq!(rows[1].up, vec![(0, 0)]);
		assert_eq!(rows[1].down, vec![(1, 1), (0, 0)]);
		assert_eq!(rows[2].node, 0);
		assert_eq!(rows[3].node, 1, "y keeps its column after x's lane ends");
	}
}
