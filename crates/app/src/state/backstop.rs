//! The 60 second backstop behind the file-system watcher: every row's status re-read in place,
//! one pass at a time, with the next deadline counted from the end of the last pass or scan
//! rather than its start, so a slow pass never starts a second one beside it (Roger 5Q-1 and
//! Morticia L-1 on 39951dc).

use std::time::Instant;

use eframe::egui;

use super::{AppState, BACKSTOP, scan};

impl AppState {
	/// Called every frame; asks for a repaint at the next deadline so the timer runs while the
	/// window is idle. Nothing starts while a scan or an earlier pass is still running.
	pub fn tick(&mut self, ctx: &egui::Context) {
		self.pump_watch(ctx);
		let now = Instant::now();
		let since = *self.backstop_at.get_or_insert(now);
		if self.scanning || self.backstop_running || self.repos.is_empty() {
			ctx.request_repaint_after(BACKSTOP);
			return;
		}
		let elapsed = now.duration_since(since);
		if elapsed >= BACKSTOP {
			self.backstop_running = true;
			scan::refresh_all(self.generation, &self.repos, self.tx.clone(), ctx.clone());
			ctx.request_repaint_after(BACKSTOP);
		} else {
			ctx.request_repaint_after(BACKSTOP - elapsed);
		}
	}

	/// A pass or a full scan finished under the current generation: the clock restarts here.
	/// Watcher caught up is claimed only now, when the pass that answered an overflow has
	/// completed, and it stands until the next pass or scan completes (Roger 5Q-2 on 7e70f1f);
	/// a full re-read that was reported meanwhile starts at once.
	pub(super) fn pass_done(&mut self, offset: Option<i32>, ctx: &egui::Context) {
		self.backstop_running = false;
		self.backstop_at = Some(Instant::now());
		self.checked_at = Some(std::time::SystemTime::now());
		self.utc_offset = offset;
		self.watching.caught_up = self.watching.overflow_pass;
		self.watching.overflow_pass = false;
		self.start_full_reread_if_due(ctx);
	}
}
