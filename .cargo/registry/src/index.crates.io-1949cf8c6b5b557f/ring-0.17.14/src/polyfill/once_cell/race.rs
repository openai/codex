//! Thread-safe, non-blocking, "first one wins" flavor of `OnceCell`.
//!
//! If two threads race to initialize a type from the `race` module, they
//! don't block, execute initialization function together, but only one of
//! them stores the result.
//!
//! This module does not require `std` feature.
//!
//! # Atomic orderings
//!
//! All types in this module use `Acquire` and `Release`
//! [atomic orderings](Ordering) for all their operations. While this is not
//! strictly necessary for types other than `OnceBox`, it is useful for users as
//! it allows them to be certain that after `get` or `get_or_init` returns on
//! one thread, any side-effects caused by the setter thread prior to them
//! calling `set` or `get_or_init` will be made visible to that thread; without
//! it, it's possible for it to appear as if they haven't happened yet from the
//! getter thread's perspective. This is an acceptable tradeoff to make since
//! `Acquire` and `Release` have very little performance overhead on most
//! architectures versus `Relaxed`.

use core::sync::atomic;

use atomic::{AtomicUsize, Ordering};
use core::num::NonZeroUsize;

/// A thread-safe cell which can be written to only once.
pub struct OnceNonZeroUsize {
    inner: AtomicUsize,
}

impl OnceNonZeroUsize {
    /// Creates a new empty cell.
    #[inline]
    pub const fn new() -> OnceNonZeroUsize {
        OnceNonZeroUsize {
            inner: AtomicUsize::new(0),
        }
    }

    /// Gets the underlying value.
    #[inline]
    pub fn get(&self) -> Option<NonZeroUsize> {
        let val = self.inner.load(Ordering::Acquire);
        NonZeroUsize::new(val)
    }

    /// Gets the contents of the cell, initializing it with `f` if the cell was
    /// empty.
    ///
    /// If several threads concurrently run `get_or_init`, more than one `f` can
    /// be called. However, all threads will return the same value, produced by
    /// some `f`.
    pub fn get_or_init<F>(&self, f: F) -> NonZeroUsize
    where
        F: FnOnce() -> NonZeroUsize,
    {
        let val = self.inner.load(Ordering::Acquire);
        match NonZeroUsize::new(val) {
            Some(it) => it,
            None => self.init(f),
        }
    }

    #[cold]
    #[inline(never)]
    fn init(&self, f: impl FnOnce() -> NonZeroUsize) -> NonZeroUsize {
        let mut val = f().get();
        let exchange = self
            .inner
            .compare_exchange(0, val, Ordering::AcqRel, Ordering::Acquire);
        if let Err(old) = exchange {
            val = old;
        }
        unsafe { NonZeroUsize::new_unchecked(val) }
    }
}
