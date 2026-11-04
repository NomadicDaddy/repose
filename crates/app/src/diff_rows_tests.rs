//! Tests for diff_rows: word marks, pairing and the inline layout.

use super::*;

#[test]
fn word_marks_cover_only_the_changed_words() {
	let (o, n) = word_marks("let x = one", "let x = two");
	assert_eq!((o.len(), o[0].clone()), (1, 8..11));
	assert_eq!((n.len(), n[0].clone()), (1, 8..11));
	assert_eq!(word_marks("same", "same"), (vec![], vec![]));
	assert_eq!(
		word_marks("abc", "xyz"),
		(vec![], vec![]),
		"nothing in common: no marks"
	);
}

#[test]
fn long_lines_get_no_marks_and_return_at_once() {
	let old: String = (0..60_000).map(|i| format!("w{i} ")).collect();
	let new: String = (0..60_000)
		.map(|i| {
			if i % 2 == 0 {
				format!("w{i} ")
			} else {
				format!("x{i} ")
			}
		})
		.collect();
	let started = Instant::now();
	let (o, n) = word_marks(&old, &new);
	assert!(o.is_empty() && n.is_empty());
	assert!(
		started.elapsed() < Duration::from_millis(50),
		"{:?}",
		started.elapsed()
	);
	// Just under the cap still gets marks, within the deadline plus a margin.
	let short_old: String = (0..600).map(|i| format!("w{i} ")).collect();
	let short_new: String = (0..600)
		.map(|i| {
			if i % 2 == 0 {
				format!("w{i} ")
			} else {
				format!("x{i} ")
			}
		})
		.collect();
	assert!(short_old.len() < MAX_MARK_BYTES);
	let started = Instant::now();
	let (o, _) = word_marks(&short_old, &short_new);
	assert!(!o.is_empty());
	assert!(
		started.elapsed() < Duration::from_millis(500),
		"{:?}",
		started.elapsed()
	);
}

#[test]
fn pairs_removed_and_added_runs_by_position() {
	let mk = |kind, text: &str| DiffLine {
		kind,
		old_no: None,
		new_no: None,
		text: text.to_string(),
	};
	let hunk = Hunk {
		old_start: 10,
		old_count: 3,
		new_start: 10,
		new_count: 4,
		heading: String::new(),
		lines: vec![
			mk(LineKind::Context, "a"),
			mk(LineKind::Removed, "b one"),
			mk(LineKind::Added, "b two"),
			mk(LineKind::Added, "c"),
			mk(LineKind::Context, "d"),
		],
	};
	let file = FileDiff {
		hunks: vec![hunk],
		..FileDiff::default()
	};
	let rows = build_rows(&file, false);
	assert!(matches!(rows[0], Row::Separator { skipped: 9, .. }));
	assert_eq!(rows.len(), 5);
	match &rows[2] {
		Row::Pair {
			old: Some(o),
			new: Some(n),
			old_marks,
			..
		} => {
			assert_eq!((o.line, n.line), (1, 2));
			assert_eq!((old_marks.len(), old_marks[0].clone()), (1, 2..5));
		}
		other => panic!("{other:?}"),
	}
	assert!(matches!(
		&rows[3],
		Row::Pair {
			old: None,
			new: Some(_),
			..
		}
	));
	assert_eq!(build_rows(&file, true).len(), 6);
}
