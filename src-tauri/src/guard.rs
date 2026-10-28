//! Keeping Melue going when something goes badly wrong in one place. Panics unwind (Cargo.toml: release builds no
//! longer abort), so a panic ends only the work it happened in:
//! - a background thread's work (the sync, the scanner, presence, playback across devices…) runs in
//!   [`keep_running`]: after a panic (logged by the panic hook, logging.rs) it starts again a moment later, instead of
//!   that part of Melue being gone until a restart;
//! - a lock that was held by the panicking work is still usable ([`Locked`]): the data it guards is whatever the work
//!   left, which every lock here can live with (state that's read again, caches, channels), rather than every later
//!   user panicking too.

use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// How long a crashed background thread waits before starting again (a panic that repeats doesn't spin).
const AGAIN_AFTER: Duration = Duration::from_secs(5);

/// Runs `work` (a background thread's whole body), again after a panic. Returns once `work` returns by itself.
pub fn keep_running(what: &str, mut work: impl FnMut()) {
    while std::panic::catch_unwind(std::panic::AssertUnwindSafe(&mut work)).is_err() {
        eprintln!("melue: {what}: crashed; started again in a moment");
        std::thread::sleep(AGAIN_AFTER);
    }
}

/// `lock()`, also when a panic left the lock poisoned.
pub trait Locked<T> {
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T> Locked<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn a_crashed_thread_starts_again_and_a_poisoned_lock_still_works() {
        let lock = Arc::new(Mutex::new(0));
        let poisoner = lock.clone();
        let _ = std::thread::spawn(move || {
            let _held = poisoner.lock().unwrap();
            panic!("on purpose");
        })
        .join();
        assert!(lock.is_poisoned());
        *lock.locked() += 1;
        assert_eq!(*lock.locked(), 1);

        // Panics once, then finishes: run twice. (The wait between is AGAIN_AFTER: kept short here by panicking first.)
        let runs = AtomicU32::new(0);
        keep_running("a test", || {
            if runs.fetch_add(1, Ordering::SeqCst) == 0 {
                panic!("first time");
            }
        });
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }
}
