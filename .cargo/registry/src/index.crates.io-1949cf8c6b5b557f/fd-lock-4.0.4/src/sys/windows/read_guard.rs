use std::os::windows::io::AsHandle;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Storage::FileSystem::UnlockFile;

use std::ops;
use std::os::windows::prelude::*;

use super::utils::syscall;
use super::RwLock;

#[derive(Debug)]
pub struct RwLockReadGuard<'lock, T: AsHandle> {
    pub(crate) lock: &'lock RwLock<T>,
}

impl<T: AsHandle> ops::Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.lock.inner
    }
}

impl<T: AsHandle> Drop for RwLockReadGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        let handle = self.lock.inner.as_handle().as_raw_handle() as HANDLE;
        let _ = syscall(unsafe { UnlockFile(handle, 0, 0, 1, 0) });
    }
}
