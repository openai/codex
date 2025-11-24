use rustix::fd::AsFd;
use rustix::fs::FlockOperation;
use std::ops;

use super::{compatible_unix_lock, RwLock};

#[derive(Debug)]
pub struct RwLockWriteGuard<'lock, T: AsFd> {
    lock: &'lock mut RwLock<T>,
}

impl<'lock, T: AsFd> RwLockWriteGuard<'lock, T> {
    pub(crate) fn new(lock: &'lock mut RwLock<T>) -> Self {
        Self { lock }
    }
}

impl<T: AsFd> ops::Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.lock.inner
    }
}

impl<T: AsFd> ops::DerefMut for RwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.lock.inner
    }
}

impl<T: AsFd> Drop for RwLockWriteGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        let _ = compatible_unix_lock(self.lock.inner.as_fd(), FlockOperation::Unlock).ok();
    }
}
