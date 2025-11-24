use std::{
    ops::Deref,
    os::unix::io::{
        AsFd,
        AsRawFd,
        BorrowedFd,
        FromRawFd,
        IntoRawFd,
        RawFd,
    },
    sync::atomic::{
        AtomicBool,
        Ordering,
    },
};

use inotify_sys as ffi;


/// A RAII guard around a `RawFd` that closes it automatically on drop.
#[derive(Debug)]
pub struct FdGuard {
    pub(crate) fd           : RawFd,
    pub(crate) close_on_drop: AtomicBool,
}

impl FdGuard {

    /// Indicate that the wrapped file descriptor should _not_ be closed
    /// when the guard is dropped.
    ///
    /// This should be called in cases where ownership of the wrapped file
    /// descriptor has been "moved" out of the guard.
    ///
    /// This is factored out into a separate function to ensure that it's
    /// always used consistently.
    #[inline]
    pub fn should_not_close(&self) {
        self.close_on_drop.store(false, Ordering::Release);
    }
}

impl Deref for FdGuard {
    type Target = RawFd;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.fd
    }
}

impl Drop for FdGuard {
    fn drop(&mut self) {
        if self.close_on_drop.load(Ordering::Acquire) {
            unsafe { ffi::close(self.fd); }
        }
    }
}

impl FromRawFd for FdGuard {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        FdGuard {
            fd,
            close_on_drop: AtomicBool::new(true),
        }
    }
}

impl IntoRawFd for FdGuard {
    fn into_raw_fd(self) -> RawFd {
        self.should_not_close();
        self.fd
    }
}

impl AsRawFd for FdGuard {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl AsFd for FdGuard {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(self.fd) }
    }
}

impl PartialEq for FdGuard {
    fn eq(&self, other: &FdGuard) -> bool {
        self.fd == other.fd
    }
}
