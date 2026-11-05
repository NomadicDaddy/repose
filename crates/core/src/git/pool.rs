//! Two bounded pools of git processes, each at most `POOL_SIZE` at once across the whole app: one
//! for reads, so a scan of many repositories never floods the machine, and one for writes, whose
//! commits wait on hooks without a time limit. A write never holds a read permit, so long hooks
//! cannot stall status, diff, history, branch or file reads (Phillip #33648). The app's write
//! queue also allows one write per repository. A permit is taken before each spawn and returned
//! when it is dropped.

use std::sync::{Condvar, Mutex, OnceLock};

pub const POOL_SIZE: usize = 8;

struct Pool {
	free: Mutex<usize>,
	cv: Condvar,
}

fn pool(write: bool) -> &'static Pool {
	static READS: OnceLock<Pool> = OnceLock::new();
	static WRITES: OnceLock<Pool> = OnceLock::new();
	let lock = if write { &WRITES } else { &READS };
	lock.get_or_init(|| Pool {
		free: Mutex::new(POOL_SIZE),
		cv: Condvar::new(),
	})
}

pub(super) struct Permit {
	write: bool,
}

impl Permit {
	/// A permit from the write pool when `write`, else from the read pool; waits for a free one.
	pub(super) fn acquire(write: bool) -> Permit {
		let p = pool(write);
		let mut free = p.free.lock().unwrap_or_else(|e| e.into_inner());
		while *free == 0 {
			free = p.cv.wait(free).unwrap_or_else(|e| e.into_inner());
		}
		*free -= 1;
		Permit { write }
	}
}

impl Drop for Permit {
	fn drop(&mut self) {
		let p = pool(self.write);
		let mut free = p.free.lock().unwrap_or_else(|e| e.into_inner());
		*free += 1;
		p.cv.notify_one();
	}
}
