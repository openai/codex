//! A minimal adaption of the `parking_lot` synchronization primitives to the
//! equivalent `std::sync` types.
//!
//! This can be extended to additional types/methods as required.

use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::LockResult;
use std::time::Duration;

// All types in this file are marked with PhantomData to ensure that
// parking_lot's send_guard feature does not leak through and affect when Tokio
// types are Send.
//
// See <https://github.com/tokio-rs/tokio/pull/4359> for more info.

// Types that do not need wrapping
pub(crate) use parking_lot::WaitTimeoutResult;

#[derive(Debug)]
pub(crate) struct Mutex<T: ?Sized>(PhantomData<std::sync::Mutex<T>>, parking_lot::Mutex<T>);

#[derive(Debug)]
pub(crate) struct RwLock<T>(PhantomData<std::sync::RwLock<T>>, parking_lot::RwLock<T>);

#[derive(Debug)]
pub(crate) struct Condvar(PhantomData<std::sync::Condvar>, parking_lot::Condvar);

#[derive(Debug)]
pub(crate) struct MutexGuard<'a, T: ?Sized>(
    PhantomData<std::sync::MutexGuard<'a, T>>,
    parking_lot::MutexGuard<'a, T>,
);

#[derive(Debug)]
pub(crate) struct RwLockReadGuard<'a, T: ?Sized>(
    PhantomData<std::sync::RwLockReadGuard<'a, T>>,
    parking_lot::RwLockReadGuard<'a, T>,
);

#[derive(Debug)]
pub(crate) struct RwLockWriteGuard<'a, T: ?Sized>(
    PhantomData<std::sync::RwLockWriteGuard<'a, T>>,
    parking_lot::RwLockWriteGuard<'a, T>,
);

impl<T> Mutex<T> {
    #[inline]
    pub(crate) fn new(t: T) -> Mutex<T> {
        Mutex(PhantomData, parking_lot::Mutex::new(t))
    }

    #[inline]
    #[cfg(not(all(loom, test)))]
    pub(crate) const fn const_new(t: T) -> Mutex<T> {
        Mutex(PhantomData, parking_lot::const_mutex(t))
    }

    #[inline]
    pub(crate) fn lock(&self) -> MutexGuard<'_, T> {
        MutexGuard(PhantomData, self.1.lock())
    }

    #[inline]
    pub(crate) fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        self.1
            .try_lock()
            .map(|guard| MutexGuard(PhantomData, guard))
    }

    #[inline]
    pub(crate) fn get_mut(&mut self) -> &mut T {
        self.1.get_mut()
    }

    // Note: Additional methods `is_poisoned` and `into_inner`, can be
    // provided here as needed.
}

impl<'a, T: ?Sized> Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.1.deref()
    }
}

impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.1.deref_mut()
    }
}

impl<T> RwLock<T> {
    pub(crate) fn new(t: T) -> RwLock<T> {
        RwLock(PhantomData, parking_lot::RwLock::new(t))
    }

    pub(crate) fn read(&self) -> RwLockReadGuard<'_, T> {
        RwLockReadGuard(PhantomData, self.1.read())
    }

    pub(crate) fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        Some(RwLockReadGuard(PhantomData, self.1.read()))
    }

    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, T> {
        RwLockWriteGuard(PhantomData, self.1.write())
    }

    pub(crate) fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        Some(RwLockWriteGuard(PhantomData, self.1.write()))
    }
}

impl<'a, T: ?Sized> Deref for RwLockReadGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.1.deref()
    }
}

impl<'a, T: ?Sized> Deref for RwLockWriteGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.1.deref()
    }
}

impl<'a, T: ?Sized> DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.1.deref_mut()
    }
}

impl Condvar {
    #[inline]
    pub(crate) fn new() -> Condvar {
        Condvar(PhantomData, parking_lot::Condvar::new())
    }

    #[inline]
    pub(crate) fn notify_one(&self) {
        self.1.notify_one();
    }

    #[inline]
    pub(crate) fn notify_all(&self) {
        self.1.notify_all();
    }

    #[inline]
    pub(crate) fn wait<'a, T>(
        &self,
        mut guard: MutexGuard<'a, T>,
    ) -> LockResult<MutexGuard<'a, T>> {
        self.1.wait(&mut guard.1);
        Ok(guard)
    }

    #[inline]
    pub(crate) fn wait_timeout<'a, T>(
        &self,
        mut guard: MutexGuard<'a, T>,
        timeout: Duration,
    ) -> LockResult<(MutexGuard<'a, T>, WaitTimeoutResult)> {
        let wtr = self.1.wait_for(&mut guard.1, timeout);
        Ok((guard, wtr))
    }

    // Note: Additional methods `wait_timeout_ms`, `wait_timeout_until`,
    // `wait_until` can be provided here as needed.
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.1, f)
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for RwLockReadGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.1, f)
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for RwLockWriteGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.1, f)
    }
}
