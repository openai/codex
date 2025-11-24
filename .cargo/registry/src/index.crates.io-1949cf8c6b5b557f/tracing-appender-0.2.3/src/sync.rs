//! Abstracts over sync primitive implementations.
//!
//! Optionally, we allow the Rust standard library's `RwLock` to be replaced
//! with the `parking_lot` crate's implementation. This may provide improved
//! performance in some cases. However, the `parking_lot` dependency is an
//! opt-in feature flag. Because `parking_lot::RwLock` has a slightly different
//! API than `std::sync::RwLock` (it does not support poisoning on panics), we
//! wrap the `std::sync` version to ignore poisoning.

#[allow(unused_imports)] // may be used later;
#[cfg(feature = "parking_lot")]
pub(crate) use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[cfg(not(feature = "parking_lot"))]
pub(crate) use self::std_impl::*;

#[cfg(not(feature = "parking_lot"))]
mod std_impl {
    use std::sync::{self, PoisonError, TryLockError};
    pub(crate) use std::sync::{RwLockReadGuard, RwLockWriteGuard};

    #[derive(Debug)]
    pub(crate) struct RwLock<T> {
        inner: sync::RwLock<T>,
    }

    impl<T> RwLock<T> {
        pub(crate) fn new(val: T) -> Self {
            Self {
                inner: sync::RwLock::new(val),
            }
        }

        #[inline]
        pub(crate) fn get_mut(&mut self) -> &mut T {
            self.inner.get_mut().unwrap_or_else(PoisonError::into_inner)
        }

        #[inline]
        pub(crate) fn read(&self) -> RwLockReadGuard<'_, T> {
            self.inner.read().unwrap_or_else(PoisonError::into_inner)
        }

        #[inline]
        #[allow(dead_code)] // may be used later;
        pub(crate) fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
            match self.inner.try_read() {
                Ok(guard) => Some(guard),
                Err(TryLockError::Poisoned(e)) => Some(e.into_inner()),
                Err(TryLockError::WouldBlock) => None,
            }
        }

        #[inline]
        pub(crate) fn write(&self) -> RwLockWriteGuard<'_, T> {
            self.inner.write().unwrap_or_else(PoisonError::into_inner)
        }

        #[inline]
        #[allow(dead_code)] // may be used later;
        pub(crate) fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
            match self.inner.try_write() {
                Ok(guard) => Some(guard),
                Err(TryLockError::Poisoned(e)) => Some(e.into_inner()),
                Err(TryLockError::WouldBlock) => None,
            }
        }
    }
}
