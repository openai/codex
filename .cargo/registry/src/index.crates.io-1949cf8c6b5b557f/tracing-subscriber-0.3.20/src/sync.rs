//! Abstracts over sync primitive implementations.
//!
//! Optionally, we allow the Rust standard library's `RwLock` to be replaced
//! with the `parking_lot` crate's implementation. This may provide improved
//! performance in some cases. However, the `parking_lot` dependency is an
//! opt-in feature flag. Because `parking_lot::RwLock` has a slightly different
//! API than `std::sync::RwLock` (it does not support poisoning on panics), we
//! wrap it with a type that provides the same method signatures. This allows us
//! to transparently swap `parking_lot` in without changing code at the callsite.
#[allow(unused_imports)] // may be used later;
pub(crate) use std::sync::{LockResult, PoisonError, TryLockResult};

#[cfg(not(feature = "parking_lot"))]
pub(crate) use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[cfg(feature = "parking_lot")]
pub(crate) use self::parking_lot_impl::*;

#[cfg(feature = "parking_lot")]
mod parking_lot_impl {
    pub(crate) use parking_lot::{RwLockReadGuard, RwLockWriteGuard};
    use std::sync::{LockResult, TryLockError, TryLockResult};

    #[derive(Debug)]
    pub(crate) struct RwLock<T> {
        inner: parking_lot::RwLock<T>,
    }

    impl<T> RwLock<T> {
        pub(crate) fn new(val: T) -> Self {
            Self {
                inner: parking_lot::RwLock::new(val),
            }
        }

        #[inline]
        pub(crate) fn get_mut(&mut self) -> LockResult<&mut T> {
            Ok(self.inner.get_mut())
        }

        #[inline]
        pub(crate) fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
            Ok(self.inner.read())
        }

        #[inline]
        #[allow(dead_code)] // may be used later;
        pub(crate) fn try_read(&self) -> TryLockResult<RwLockReadGuard<'_, T>> {
            self.inner.try_read().ok_or(TryLockError::WouldBlock)
        }

        #[inline]
        pub(crate) fn write(&self) -> LockResult<RwLockWriteGuard<'_, T>> {
            Ok(self.inner.write())
        }
    }

    impl<T: Default> Default for RwLock<T> {
        fn default() -> Self {
            RwLock {
                inner: parking_lot::RwLock::default(),
            }
        }
    }
}
