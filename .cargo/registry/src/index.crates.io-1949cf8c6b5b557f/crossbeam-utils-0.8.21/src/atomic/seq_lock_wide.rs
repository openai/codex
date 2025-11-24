use core::mem;
use core::sync::atomic::{self, AtomicUsize, Ordering};

use crate::Backoff;

/// A simple stamped lock.
///
/// The state is represented as two `AtomicUsize`: `state_hi` for high bits and `state_lo` for low
/// bits.
pub(crate) struct SeqLock {
    /// The high bits of the current state of the lock.
    state_hi: AtomicUsize,

    /// The low bits of the current state of the lock.
    ///
    /// All bits except the least significant one hold the current stamp. When locked, the state_lo
    /// equals 1 and doesn't contain a valid stamp.
    state_lo: AtomicUsize,
}

impl SeqLock {
    pub(crate) const fn new() -> Self {
        Self {
            state_hi: AtomicUsize::new(0),
            state_lo: AtomicUsize::new(0),
        }
    }

    /// If not locked, returns the current stamp.
    ///
    /// This method should be called before optimistic reads.
    #[inline]
    pub(crate) fn optimistic_read(&self) -> Option<(usize, usize)> {
        // The acquire loads from `state_hi` and `state_lo` synchronize with the release stores in
        // `SeqLockWriteGuard::drop`.
        //
        // As a consequence, we can make sure that (1) all writes within the era of `state_hi - 1`
        // happens before now; and therefore, (2) if `state_lo` is even, all writes within the
        // critical section of (`state_hi`, `state_lo`) happens before now.
        let state_hi = self.state_hi.load(Ordering::Acquire);
        let state_lo = self.state_lo.load(Ordering::Acquire);
        if state_lo == 1 {
            None
        } else {
            Some((state_hi, state_lo))
        }
    }

    /// Returns `true` if the current stamp is equal to `stamp`.
    ///
    /// This method should be called after optimistic reads to check whether they are valid. The
    /// argument `stamp` should correspond to the one returned by method `optimistic_read`.
    #[inline]
    pub(crate) fn validate_read(&self, stamp: (usize, usize)) -> bool {
        // Thanks to the fence, if we're noticing any modification to the data at the critical
        // section of `(a, b)`, then the critical section's write of 1 to state_lo should be
        // visible.
        atomic::fence(Ordering::Acquire);

        // So if `state_lo` coincides with `stamp.1`, then either (1) we're noticing no modification
        // to the data after the critical section of `(stamp.0, stamp.1)`, or (2) `state_lo` wrapped
        // around.
        //
        // If (2) is the case, the acquire ordering ensures we see the new value of `state_hi`.
        let state_lo = self.state_lo.load(Ordering::Acquire);

        // If (2) is the case and `state_hi` coincides with `stamp.0`, then `state_hi` also wrapped
        // around, which we give up to correctly validate the read.
        let state_hi = self.state_hi.load(Ordering::Relaxed);

        // Except for the case that both `state_hi` and `state_lo` wrapped around, the following
        // condition implies that we're noticing no modification to the data after the critical
        // section of `(stamp.0, stamp.1)`.
        (state_hi, state_lo) == stamp
    }

    /// Grabs the lock for writing.
    #[inline]
    pub(crate) fn write(&'static self) -> SeqLockWriteGuard {
        let backoff = Backoff::new();
        loop {
            let previous = self.state_lo.swap(1, Ordering::Acquire);

            if previous != 1 {
                // To synchronize with the acquire fence in `validate_read` via any modification to
                // the data at the critical section of `(state_hi, previous)`.
                atomic::fence(Ordering::Release);

                return SeqLockWriteGuard {
                    lock: self,
                    state_lo: previous,
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
    state_lo: usize,
}

impl SeqLockWriteGuard {
    /// Releases the lock without incrementing the stamp.
    #[inline]
    pub(crate) fn abort(self) {
        self.lock.state_lo.store(self.state_lo, Ordering::Release);
        mem::forget(self);
    }
}

impl Drop for SeqLockWriteGuard {
    #[inline]
    fn drop(&mut self) {
        let state_lo = self.state_lo.wrapping_add(2);

        // Increase the high bits if the low bits wrap around.
        //
        // Release ordering for synchronizing with `optimistic_read`.
        if state_lo == 0 {
            let state_hi = self.lock.state_hi.load(Ordering::Relaxed);
            self.lock
                .state_hi
                .store(state_hi.wrapping_add(1), Ordering::Release);
        }

        // Release the lock and increment the stamp.
        //
        // Release ordering for synchronizing with `optimistic_read`.
        self.lock.state_lo.store(state_lo, Ordering::Release);
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
