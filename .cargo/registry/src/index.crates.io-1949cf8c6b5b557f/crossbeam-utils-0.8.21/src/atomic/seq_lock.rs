use core::mem;
use core::sync::atomic::{self, AtomicUsize, Ordering};

use crate::Backoff;

/// A simple stamped lock.
pub(crate) struct SeqLock {
    /// The current state of the lock.
    ///
    /// All bits except the least significant one hold the current stamp. When locked, the state
    /// equals 1 and doesn't contain a valid stamp.
    state: AtomicUsize,
}

impl SeqLock {
    pub(crate) const fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
        }
    }

    /// If not locked, returns the current stamp.
    ///
    /// This method should be called before optimistic reads.
    #[inline]
    pub(crate) fn optimistic_read(&self) -> Option<usize> {
        let state = self.state.load(Ordering::Acquire);
        if state == 1 {
            None
        } else {
            Some(state)
        }
    }

    /// Returns `true` if the current stamp is equal to `stamp`.
    ///
    /// This method should be called after optimistic reads to check whether they are valid. The
    /// argument `stamp` should correspond to the one returned by method `optimistic_read`.
    #[inline]
    pub(crate) fn validate_read(&self, stamp: usize) -> bool {
        atomic::fence(Ordering::Acquire);
        self.state.load(Ordering::Relaxed) == stamp
    }

    /// Grabs the lock for writing.
    #[inline]
    pub(crate) fn write(&'static self) -> SeqLockWriteGuard {
        let backoff = Backoff::new();
        loop {
            let previous = self.state.swap(1, Ordering::Acquire);

            if previous != 1 {
                atomic::fence(Ordering::Release);

                return SeqLockWriteGuard {
                    lock: self,
                    state: previous,
                };
            }

            backoff.snooze();
        }
    }
}

/// An RAII guard that releases the lock and increments the stamp when dropped.
pub(crate) struct SeqLockWriteGuard {
    /// The parent lock.
    lock: &'static SeqLock,

    /// The stamp before locking.
    state: usize,
}

impl SeqLockWriteGuard {
    /// Releases the lock without incrementing the stamp.
    #[inline]
    pub(crate) fn abort(self) {
        self.lock.state.store(self.state, Ordering::Release);

        // We specifically don't want to call drop(), since that's
        // what increments the stamp.
        mem::forget(self);
    }
}

impl Drop for SeqLockWriteGuard {
    #[inline]
    fn drop(&mut self) {
        // Release the lock and increment the stamp.
        self.lock
            .state
            .store(self.state.wrapping_add(2), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::SeqLock;

    #[test]
    fn test_abort() {
        static LK: SeqLock = SeqLock::new();
        let before = LK.optimistic_read().unwrap();
        {
            let guard = LK.write();
            guard.abort();
        }
        let after = LK.optimistic_read().unwrap();
        assert_eq!(before, after, "aborted write does not update the stamp");
    }
}
