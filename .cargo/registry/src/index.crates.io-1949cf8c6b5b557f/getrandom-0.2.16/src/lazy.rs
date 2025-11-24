use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

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
pub(crate) struct LazyUsize(AtomicUsize);

impl LazyUsize {
    pub const fn new() -> Self {
        Self(AtomicUsize::new(Self::UNINIT))
    }

    // The initialization is not completed.
    pub const UNINIT: usize = usize::max_value();

    // Runs the init() function at most once, returning the value of some run of
    // init(). Multiple callers can run their init() functions in parallel.
    // init() should always return the same value, if it succeeds.
    pub fn unsync_init(&self, init: impl FnOnce() -> usize) -> usize {
        // Relaxed ordering is fine, as we only have a single atomic variable.
        let mut val = self.0.load(Relaxed);
        if val == Self::UNINIT {
            val = init();
            self.0.store(val, Relaxed);
        }
        val
    }
}

// Identical to LazyUsize except with bool instead of usize.
pub(crate) struct LazyBool(LazyUsize);

impl LazyBool {
    pub const fn new() -> Self {
        Self(LazyUsize::new())
    }

    pub fn unsync_init(&self, init: impl FnOnce() -> bool) -> bool {
        self.0.unsync_init(|| init() as usize) != 0
    }
}
