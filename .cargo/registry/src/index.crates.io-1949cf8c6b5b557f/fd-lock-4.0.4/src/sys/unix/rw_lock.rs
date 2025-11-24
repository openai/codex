use rustix::fd::AsFd;
use rustix::fs::FlockOperation;
use std::io::{self, Error, ErrorKind};

use super::{compatible_unix_lock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug)]
pub struct RwLock<T: AsFd> {
    pub(crate) inner: T,
}

impl<T: AsFd> RwLock<T> {
    #[inline]
    pub fn new(inner: T) -> Self {
        RwLock { inner }
    }

    #[inline]
    pub fn write(&mut self) -> io::Result<RwLockWriteGuard<'_, T>> {
        compatible_unix_lock(self.inner.as_fd(), FlockOperation::LockExclusive)?;
        Ok(RwLockWriteGuard::new(self))
    }

    #[inline]
    pub fn try_write(&mut self) -> Result<RwLockWriteGuard<'_, T>, Error> {
        compatible_unix_lock(self.inner.as_fd(), FlockOperation::NonBlockingLockExclusive)
            .map_err(|err| match err.kind() {
                ErrorKind::AlreadyExists => ErrorKind::WouldBlock.into(),
                _ => Error::from(err),
            })?;
        Ok(RwLockWriteGuard::new(self))
    }

    #[inline]
    pub fn read(&self) -> io::Result<RwLockReadGuard<'_, T>> {
        compatible_unix_lock(self.inner.as_fd(), FlockOperation::LockShared)?;
        Ok(RwLockReadGuard::new(self))
    }

    #[inline]
    pub fn try_read(&self) -> Result<RwLockReadGuard<'_, T>, Error> {
        compatible_unix_lock(self.inner.as_fd(), FlockOperation::NonBlockingLockShared).map_err(
            |err| match err.kind() {
                ErrorKind::AlreadyExists => ErrorKind::WouldBlock.into(),
                _ => Error::from(err),
            },
        )?;
        Ok(RwLockReadGuard::new(self))
    }

    #[inline]
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        self.inner
    }
}
