use std::os::windows::io::AsHandle;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Storage::FileSystem::UnlockFile;

use std::ops;
use std::os::windows::prelude::*;

use super::utils::syscall;
use super::RwLock;

#[derive(Debug)]
pub struct RwLockWriteGuard<'lock, T: AsHandle> {
    pub(crate) lock: &'lock mut RwLock<T>,
}

impl<T: AsHandle> ops::Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.lock.inner
    }
}

impl<T: AsHandle> ops::DerefMut for RwLockWriteGuard<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.lock.inner
    }
}

impl<T: AsHandle> Drop for RwLockWriteGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        let handle = self.lock.inner.as_handle().as_raw_handle() as HANDLE;
        let _ = syscall(unsafe { UnlockFile(handle, 0, 0, 1, 0) });
    }
}
