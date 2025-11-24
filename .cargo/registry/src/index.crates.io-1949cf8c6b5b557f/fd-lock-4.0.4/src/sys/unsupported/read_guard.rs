use std::ops;
use std::os::unix::io::AsRawFd;

use super::RwLock;

#[derive(Debug)]
pub struct RwLockReadGuard<'lock, T: AsRawFd> {
    lock: &'lock RwLock<T>,
}

impl<'lock, T: AsRawFd> RwLockReadGuard<'lock, T> {
    pub(crate) fn new(lock: &'lock RwLock<T>) -> Self {
        panic!("target unsupported")
    }
}

impl<T: AsRawFd> ops::Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        panic!("target unsupported")
    }
}

impl<T: AsRawFd> Drop for RwLockReadGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        panic!("target unsupported")
    }
}
