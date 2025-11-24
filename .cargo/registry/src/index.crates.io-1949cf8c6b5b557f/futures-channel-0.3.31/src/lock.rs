//! A "mutex" which only supports `try_lock`
//!
//! As a futures library the eventual call to an event loop should be the only
//! thing that ever blocks, so this is assisted with a fast user-space
//! implementation of a lock that can only have a `try_lock` operation.

use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::SeqCst;

/// A "mutex" around a value, similar to `std::sync::Mutex<T>`.
///
/// This lock only supports the `try_lock` operation, however, and does not
/// implement poisoning.
#[derive(Debug)]
pub(crate) struct Lock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

/// Sentinel representing an acquired lock through which the data can be
/// accessed.
pub(crate) struct TryLock<'a, T> {
    __ptr: &'a Lock<T>,
}

// The `Lock` structure is basically just a `Mutex<T>`, and these two impls are
// intended to mirror the standard library's corresponding impls for `Mutex<T>`.
//
// If a `T` is sendable across threads, so is the lock, and `T` must be sendable
// across threads to be `Sync` because it allows mutable access from multiple
// threads.
unsafe impl<T: Send> Send for Lock<T> {}
unsafe impl<T: Send> Sync for Lock<T> {}

impl<T> Lock<T> {
    /// Creates a new lock around the given value.
    pub(crate) fn new(t: T) -> Self {
        Self { locked: AtomicBool::new(false), data: UnsafeCell::new(t) }
    }

    /// Attempts to acquire this lock, returning whether the lock was acquired or
    /// not.
    ///
    /// If `Some` is returned then the data this lock protects can be accessed
    /// through the sentinel. This sentinel allows both mutable and immutable
    /// access.
    ///
    /// If `None` is returned then the lock is already locked, either elsewhere
    /// on this thread or on another thread.
    pub(crate) fn try_lock(&self) -> Option<TryLock<'_, T>> {
        if !self.locked.swap(true, SeqCst) {
            Some(TryLock { __ptr: self })
        } else {
            None
        }
    }
}

impl<T> Deref for TryLock<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // The existence of `TryLock` represents that we own the lock, so we
        // can safely access the data here.
        unsafe { &*self.__ptr.data.get() }
    }
}

impl<T> DerefMut for TryLock<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // The existence of `TryLock` represents that we own the lock, so we
        // can safely access the data here.
        //
        // Additionally, we're the *only* `TryLock` in existence so mutable
        // access should be ok.
        unsafe { &mut *self.__ptr.data.get() }
    }
}

impl<T> Drop for TryLock<'_, T> {
    fn drop(&mut self) {
        self.__ptr.locked.store(false, SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::Lock;

    #[test]
    fn smoke() {
        let a = Lock::new(1);
        let mut a1 = a.try_lock().unwrap();
        assert!(a.try_lock().is_none());
        assert_eq!(*a1, 1);
        *a1 = 2;
        drop(a1);
        assert_eq!(*a.try_lock().unwrap(), 2);
        assert_eq!(*a.try_lock().unwrap(), 2);
    }
}
