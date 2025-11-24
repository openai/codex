use std::sync::{self, MutexGuard, TryLockError};

/// Adapter for `std::Mutex` that removes the poisoning aspects
/// from its API.
#[derive(Debug)]
pub(crate) struct Mutex<T: ?Sized>(sync::Mutex<T>);

#[allow(dead_code)]
impl<T> Mutex<T> {
    #[inline]
    pub(crate) fn new(t: T) -> Mutex<T> {
        Mutex(sync::Mutex::new(t))
    }

    #[inline]
    pub(crate) const fn const_new(t: T) -> Mutex<T> {
        Mutex(sync::Mutex::new(t))
    }

    #[inline]
    pub(crate) fn lock(&self) -> MutexGuard<'_, T> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(p_err) => p_err.into_inner(),
        }
    }

    #[inline]
    pub(crate) fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        match self.0.try_lock() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(p_err)) => Some(p_err.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }
}
