//! A bounded pool of git processes: at most `POOL_SIZE` at once across the whole app, so a scan
//! of many repositories never floods the machine. A permit is taken before each spawn and
//! returned when it is dropped.

use std::sync::{Condvar, Mutex, OnceLock};

pub const POOL_SIZE: usize = 8;

struct Pool {
	free: Mutex<usize>,
	cv: Condvar,
}

fn pool() -> &'static Pool {
	static POOL: OnceLock<Pool> = OnceLock::new();
	POOL.get_or_init(|| Pool {
		free: Mutex::new(POOL_SIZE),
		cv: Condvar::new(),
	})
}

pub(super) struct Permit;

impl Permit {
	pub(super) fn acquire() -> Permit {
		let p = pool();
		let mut free = p.free.lock().unwrap_or_else(|e| e.into_inner());
		while *free == 0 {
			free = p.cv.wait(free).unwrap_or_else(|e| e.into_inner());
		}
		*free -= 1;
		Permit
	}
}

impl Drop for Permit {
	fn drop(&mut self) {
		let p = pool();
		let mut free = p.free.lock().unwrap_or_else(|e| e.into_inner());
		*free += 1;
		p.cv.notify_one();
	}
}
