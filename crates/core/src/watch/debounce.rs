//! One refresh per repository per quiet window: the first event starts a 250 ms wait and every
//! later one restarts it, so a build that touches a thousand files produces one refresh when it
//! goes quiet. A storm that never goes quiet (a long install) still gets a refresh every
//! `MAX_WAIT`, so the row is never more than two seconds behind.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const WINDOW: Duration = Duration::from_millis(250);
pub const MAX_WAIT: Duration = Duration::from_secs(2);

struct Pending {
	first: Instant,
	last: Instant,
}

#[derive(Default)]
pub struct Debouncer {
	pending: HashMap<PathBuf, Pending>,
}

impl Debouncer {
	/// Records an event for a repository at `now`.
	pub fn note(&mut self, repo: PathBuf, now: Instant) {
		self.pending
			.entry(repo)
			.and_modify(|p| p.last = now)
			.or_insert(Pending {
				first: now,
				last: now,
			});
	}

	/// The repositories whose window has closed at `now`, each removed as it is returned.
	pub fn due(&mut self, now: Instant) -> Vec<PathBuf> {
		let mut out: Vec<PathBuf> = self
			.pending
			.iter()
			.filter(|(_, p)| now >= p.last + WINDOW || now >= p.first + MAX_WAIT)
			.map(|(repo, _)| repo.clone())
			.collect();
		out.sort();
		for repo in &out {
			self.pending.remove(repo);
		}
		out
	}

	/// When the next repository could become due, to size the wait between polls.
	pub fn next_deadline(&self) -> Option<Instant> {
		self.pending
			.values()
			.map(|p| (p.last + WINDOW).min(p.first + MAX_WAIT))
			.min()
	}

	pub fn is_empty(&self) -> bool {
		self.pending.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_thousand_events_in_a_tenth_of_a_second_give_one_refresh() {
		let mut d = Debouncer::default();
		let start = Instant::now();
		let repo = PathBuf::from("D:/t/a");
		for i in 0..1000u64 {
			d.note(repo.clone(), start + Duration::from_micros(i * 100));
		}
		let during = start + Duration::from_millis(100);
		assert!(d.due(during).is_empty(), "still inside the window");
		assert_eq!(
			d.next_deadline(),
			Some(start + Duration::from_micros(999 * 100) + WINDOW)
		);
		let after = start + Duration::from_millis(400);
		assert_eq!(d.due(after), vec![repo.clone()]);
		assert!(d.due(after).is_empty(), "fired once");
		assert!(d.is_empty());
	}

	#[test]
	fn a_storm_that_never_quiets_fires_at_the_cap_and_repositories_are_independent() {
		let mut d = Debouncer::default();
		let start = Instant::now();
		let (a, b) = (PathBuf::from("D:/t/a"), PathBuf::from("D:/t/b"));
		d.note(b.clone(), start);
		for ms in (0..3000).step_by(50) {
			d.note(a.clone(), start + Duration::from_millis(ms));
		}
		let fired = d.due(start + Duration::from_millis(2000));
		assert_eq!(
			fired,
			vec![a.clone(), b.clone()],
			"a at the cap, b after its quiet window"
		);
		d.note(a.clone(), start + Duration::from_millis(2050));
		assert!(d.due(start + Duration::from_millis(2100)).is_empty());
		assert_eq!(d.due(start + Duration::from_millis(2300)), vec![a]);
	}
}
