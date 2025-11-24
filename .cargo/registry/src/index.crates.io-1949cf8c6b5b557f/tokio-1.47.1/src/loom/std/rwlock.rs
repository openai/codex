use std::sync::{self, RwLockReadGuard, RwLockWriteGuard, TryLockError};

/// Adapter for `std::sync::RwLock` that removes the poisoning aspects
/// from its api.
#[derive(Debug)]
pub(crate) struct RwLock<T: ?Sized>(sync::RwLock<T>);

#[allow(dead_code)]
impl<T> RwLock<T> {
    #[inline]
    pub(crate) fn new(t: T) -> Self {
        Self(sync::RwLock::new(t))
    }

    #[inline]
    pub(crate) fn read(&self) -> RwLockReadGuard<'_, T> {
        match self.0.read() {
            Ok(guard) => guard,
            Err(p_err) => p_err.into_inner(),
        }
    }

    #[inline]
    pub(crate) fn try_read(&self) -> Option<RwLockReadGuard<'_, T>> {
        match self.0.try_read() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(p_err)) => Some(p_err.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }

    #[inline]
    pub(crate) fn write(&self) -> RwLockWriteGuard<'_, T> {
        match self.0.write() {
            Ok(guard) => guard,
            Err(p_err) => p_err.into_inner(),
        }
    }

    #[inline]
    pub(crate) fn try_write(&self) -> Option<RwLockWriteGuard<'_, T>> {
        match self.0.try_write() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(p_err)) => Some(p_err.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }
}
