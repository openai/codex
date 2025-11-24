//! Helpers built around pointer-sized atomics.
use core::sync::atomic::{AtomicUsize, Ordering};

// This structure represents a lazily initialized static usize value. Useful
// when it is preferable to just rerun initialization instead of locking.
// unsync_init will invoke an init() function until it succeeds, then return the
// cached value for future calls.
//
// unsync_init supports init() "failing". If the init() method returns UNINIT,
// that value will be returned as normal, but will not be cached.
//
// Users should only depend on the _value_ returned by init() functions.
// Specifically, for the following init() function:
//      fn init() -> usize {
//          a();
//          let v = b();
//          c();
//          v
//      }
// the effects of c() or writes to shared memory will not necessarily be
// observed and additional synchronization methods may be needed.
struct LazyUsize(AtomicUsize);

impl LazyUsize {
    // The initialization is not completed.
    const UNINIT: usize = usize::MAX;

    const fn new() -> Self {
        Self(AtomicUsize::new(Self::UNINIT))
    }

    // Runs the init() function at most once, returning the value of some run of
    // init(). Multiple callers can run their init() functions in parallel.
    // init() should always return the same value, if it succeeds.
    fn unsync_init(&self, init: impl FnOnce() -> usize) -> usize {
        #[cold]
        fn do_init(this: &LazyUsize, init: impl FnOnce() -> usize) -> usize {
            let val = init();
            this.0.store(val, Ordering::Relaxed);
            val
        }

        // Relaxed ordering is fine, as we only have a single atomic variable.
        let val = self.0.load(Ordering::Relaxed);
        if val != Self::UNINIT {
            val
        } else {
            do_init(self, init)
        }
    }
}

// Identical to LazyUsize except with bool instead of usize.
pub(crate) struct LazyBool(LazyUsize);

impl LazyBool {
    pub const fn new() -> Self {
        Self(LazyUsize::new())
    }

    pub fn unsync_init(&self, init: impl FnOnce() -> bool) -> bool {
        self.0.unsync_init(|| usize::from(init())) != 0
    }
}
