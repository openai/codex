use std::io::{self, Error, ErrorKind};
use std::os::windows::io::{AsHandle, AsRawHandle};

use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Storage::FileSystem::{
    LockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
};

use super::utils::{syscall, Overlapped};
use super::{RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug)]
pub struct RwLock<T: AsHandle> {
    pub(crate) inner: T,
}

impl<T: AsHandle> RwLock<T> {
    #[inline]
    pub fn new(inner: T) -> Self {
        RwLock { inner }
    }

    #[inline]
    pub fn read(&self) -> io::Result<RwLockReadGuard<'_, T>> {
        // See: https://stackoverflow.com/a/9186532, https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex
        let handle = self.inner.as_handle().as_raw_handle() as HANDLE;
        let overlapped = Overlapped::zero();
        let flags = 0;
        syscall(unsafe { LockFileEx(handle, flags, 0, 1, 0, overlapped.raw()) })?;
        Ok(RwLockReadGuard { lock: self })
    }

    #[inline]
    pub fn try_read(&self) -> io::Result<RwLockReadGuard<'_, T>> {
        let handle = self.inner.as_handle().as_raw_handle() as HANDLE;
        let overlapped = Overlapped::zero();
        let flags = LOCKFILE_FAIL_IMMEDIATELY;

        syscall(unsafe { LockFileEx(handle, flags, 0, 1, 0, overlapped.raw()) }).map_err(
            |error| match error.raw_os_error().map(|error_code| error_code as u32) {
                Some(ERROR_LOCK_VIOLATION) => Error::from(ErrorKind::WouldBlock),
                _ => error,
            },
        )?;
        Ok(RwLockReadGuard { lock: self })
    }

    #[inline]
    pub fn write(&mut self) -> io::Result<RwLockWriteGuard<'_, T>> {
        // See: https://stackoverflow.com/a/9186532, https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-lockfileex
        let handle = self.inner.as_handle().as_raw_handle() as HANDLE;
        let overlapped = Overlapped::zero();
        let flags = LOCKFILE_EXCLUSIVE_LOCK;
        syscall(unsafe { LockFileEx(handle, flags, 0, 1, 0, overlapped.raw()) })?;
        Ok(RwLockWriteGuard { lock: self })
    }

    #[inline]
    pub fn try_write(&mut self) -> io::Result<RwLockWriteGuard<'_, T>> {
        let handle = self.inner.as_handle().as_raw_handle() as HANDLE;
        let overlapped = Overlapped::zero();
        let flags = LOCKFILE_FAIL_IMMEDIATELY | LOCKFILE_EXCLUSIVE_LOCK;

        syscall(unsafe { LockFileEx(handle, flags, 0, 1, 0, overlapped.raw()) }).map_err(
            |error| match error.raw_os_error().map(|error_code| error_code as u32) {
                Some(ERROR_LOCK_VIOLATION) => Error::from(ErrorKind::WouldBlock),
                _ => error,
            },
        )?;
        Ok(RwLockWriteGuard { lock: self })
    }

    #[inline]
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        self.inner
    }
}
