use std::ops;
use std::os::unix::io::AsRawFd;

use super::RwLock;

#[derive(Debug)]
pub struct RwLockWriteGuard<'lock, T: AsRawFd> {
    lock: &'lock mut RwLock<T>,
}

impl<'lock, T: AsRawFd> RwLockWriteGuard<'lock, T> {
    pub(crate) fn new(lock: &'lock mut RwLock<T>) -> Self {
        panic!("target unsupported")
    }
}

impl<T: AsRawFd> ops::Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        panic!("target unsupported")
    }
}

impl<T: AsRawFd> ops::DerefMut for RwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        panic!("target unsupported")
    }
}

impl<T: AsRawFd> Drop for RwLockWriteGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        panic!("target unsupported")
    }
}
