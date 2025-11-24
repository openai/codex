//! The following is derived from Rust's
//! library/std/src/os/fd/owned.rs at revision
//! 334a54cd83191f38ad8046ed94c45de735c86c65.
//!
//! All code in this file is licensed MIT or Apache 2.0 at your option.
//!
//! Owned and borrowed Unix-like file descriptors.

#![cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unsafe_code)]

use super::raw::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use crate::io::close;
use core::fmt;
use core::marker::PhantomData;
use core::mem::forget;

/// A borrowed file descriptor.
///
/// This has a lifetime parameter to tie it to the lifetime of something that owns the file
/// descriptor. For the duration of that lifetime, it is guaranteed that nobody will close the file
/// descriptor.
///
/// This uses `repr(transparent)` and has the representation of a host file
/// descriptor, so it can be used in FFI in places where a file descriptor is
/// passed as an argument, it is not captured or consumed, and it never has the
/// value `-1`.
///
/// This type's `.to_owned()` implementation returns another `BorrowedFd`
/// rather than an `OwnedFd`. It just makes a trivial copy of the raw file
/// descriptor, which is then borrowed under the same lifetime.
#[derive(Copy, Clone)]
#[repr(transparent)]
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_start(0))]
// libstd/os/raw/mod.rs assures me that every libstd-supported platform has a
// 32-bit c_int. Below is -2, in two's complement, but that only works out
// because c_int is 32 bits.
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FE))]
#[cfg_attr(rustc_attrs, rustc_nonnull_optimization_guaranteed)]
#[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
pub struct BorrowedFd<'fd> {
    fd: RawFd,
    _phantom: PhantomData<&'fd OwnedFd>,
}

/// An owned file descriptor.
///
/// This closes the file descriptor on drop. It is guaranteed that nobody else will close the file
/// descriptor.
///
/// This uses `repr(transparent)` and has the representation of a host file
/// descriptor, so it can be used in FFI in places where a file descriptor is
/// passed as a consumed argument or returned as an owned value, and it never
/// has the value `-1`.
#[repr(transparent)]
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_start(0))]
// libstd/os/raw/mod.rs assures me that every libstd-supported platform has a
// 32-bit c_int. Below is -2, in two's complement, but that only works out
// because c_int is 32 bits.
#[cfg_attr(rustc_attrs, rustc_layout_scalar_valid_range_end(0xFF_FF_FF_FE))]
#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
#[cfg_attr(rustc_attrs, rustc_nonnull_optimization_guaranteed)]
pub struct OwnedFd {
    fd: RawFd,
}

impl BorrowedFd<'_> {
    /// Return a `BorrowedFd` holding the given raw file descriptor.
    ///
    /// # Safety
    ///
    /// The resource pointed to by `fd` must remain open for the duration of
    /// the returned `BorrowedFd`, and it must not have the value `-1`.
    #[inline]
    #[cfg_attr(
        staged_api,
        rustc_const_stable(feature = "io_safety", since = "1.63.0")
    )]
    #[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
    pub const unsafe fn borrow_raw(fd: RawFd) -> Self {
        assert!(fd != u32::MAX as RawFd);
        // SAFETY: we just asserted that the value is in the valid range and isn't `-1` (the only value bigger than `0xFF_FF_FF_FE` unsigned)
        #[allow(unused_unsafe)]
        unsafe {
            Self {
                fd,
                _phantom: PhantomData,
            }
        }
    }
}

impl OwnedFd {
    /// Creates a new `OwnedFd` instance that shares the same underlying file handle
    /// as the existing `OwnedFd` instance.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_clone(&self) -> crate::io::Result<Self> {
        // We want to atomically duplicate this file descriptor and set the
        // CLOEXEC flag, and currently that's done via F_DUPFD_CLOEXEC. This
        // is a POSIX flag that was added to Linux in 2.6.24.
        #[cfg(not(target_os = "espidf"))]
        let fd = crate::io::fcntl_dupfd_cloexec(self, 0)?;

        // For ESP-IDF, F_DUPFD is used instead, because the CLOEXEC semantics
        // will never be supported, as this is a bare metal framework with
        // no capabilities for multi-process execution. While F_DUPFD is also
        // not supported yet, it might be (currently it returns ENOSYS).
        #[cfg(target_os = "espidf")]
        let fd = crate::io::fcntl_dupfd(self)?;

        Ok(fd.into())
    }

    /// Creates a new `OwnedFd` instance that shares the same underlying file handle
    /// as the existing `OwnedFd` instance.
    #[cfg(target_arch = "wasm32")]
    pub fn try_clone(&self) -> crate::io::Result<Self> {
        Err(crate::io::Errno::NOSYS)
    }
}

impl BorrowedFd<'_> {
    /// Creates a new `OwnedFd` instance that shares the same underlying file
    /// description as the existing `BorrowedFd` instance.
    #[cfg(not(any(target_arch = "wasm32", target_os = "hermit")))]
    #[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
    pub fn try_clone_to_owned(&self) -> crate::io::Result<OwnedFd> {
        // Avoid using file descriptors below 3 as they are used for stdio

        // We want to atomically duplicate this file descriptor and set the
        // CLOEXEC flag, and currently that's done via F_DUPFD_CLOEXEC. This
        // is a POSIX flag that was added to Linux in 2.6.24.
        #[cfg(not(target_os = "espidf"))]
        let fd = crate::io::fcntl_dupfd_cloexec(self, 3)?;

        // For ESP-IDF, F_DUPFD is used instead, because the CLOEXEC semantics
        // will never be supported, as this is a bare metal framework with
        // no capabilities for multi-process execution. While F_DUPFD is also
        // not supported yet, it might be (currently it returns ENOSYS).
        #[cfg(target_os = "espidf")]
        let fd = crate::io::fcntl_dupfd(self, 3)?;

        Ok(fd)
    }

    /// Creates a new `OwnedFd` instance that shares the same underlying file
    /// description as the existing `BorrowedFd` instance.
    #[cfg(any(target_arch = "wasm32", target_os = "hermit"))]
    #[cfg_attr(staged_api, stable(feature = "io_safety", since = "1.63.0"))]
    pub fn try_clone_to_owned(&self) -> crate::io::Result<OwnedFd> {
        Err(crate::io::Errno::NOSYS)
    }
}

#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
impl AsRawFd for BorrowedFd<'_> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
impl AsRawFd for OwnedFd {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
impl IntoRawFd for OwnedFd {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        let fd = self.fd;
        forget(self);
        fd
    }
}

#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
impl FromRawFd for OwnedFd {
    /// Constructs a new instance of `Self` from the given raw file descriptor.
    ///
    /// # Safety
    ///
    /// The resource pointed to by `fd` must be open and suitable for assuming
    /// [ownership][io-safety]. The resource must not require any cleanup other than `close`.
    ///
    /// [io-safety]: io#io-safety
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        assert_ne!(fd, u32::MAX as RawFd);
        // SAFETY: we just asserted that the value is in the valid range and isn't `-1` (the only value bigger than `0xFF_FF_FF_FE` unsigned)
        #[allow(unused_unsafe)]
        unsafe {
            Self { fd }
        }
    }
}

#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
impl Drop for OwnedFd {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // Errors are ignored when closing a file descriptor. The reason
            // for this is that if an error occurs we don't actually know if
            // the file descriptor was closed or not, and if we retried (for
            // something like EINTR), we might close another valid file
            // descriptor opened after we closed ours.
            close(self.fd as _);
        }
    }
}

#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
impl fmt::Debug for BorrowedFd<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedFd").field("fd", &self.fd).finish()
    }
}

#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
impl fmt::Debug for OwnedFd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedFd").field("fd", &self.fd).finish()
    }
}

/// A trait to borrow the file descriptor from an underlying object.
///
/// This is only available on unix platforms and must be imported in order to
/// call the method. Windows platforms have a corresponding `AsHandle` and
/// `AsSocket` set of traits.
#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
pub trait AsFd {
    /// Borrows the file descriptor.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # #![feature(io_safety)]
    /// use std::fs::File;
    /// # use std::io;
    /// # #[cfg(target_os = "wasi")]
    /// # use std::os::wasi::io::{AsFd, BorrowedFd};
    /// # #[cfg(unix)]
    /// # use std::os::unix::io::{AsFd, BorrowedFd};
    ///
    /// let mut f = File::open("foo.txt")?;
    /// # #[cfg(any(unix, target_os = "wasi"))]
    /// let borrowed_fd: BorrowedFd<'_> = f.as_fd();
    /// # Ok::<(), io::Error>(())
    /// ```
    #[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
    fn as_fd(&self) -> BorrowedFd<'_>;
}

#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
impl<T: AsFd> AsFd for &T {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        T::as_fd(self)
    }
}

#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
impl<T: AsFd> AsFd for &mut T {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        T::as_fd(self)
    }
}

#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
impl AsFd for BorrowedFd<'_> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        *self
    }
}

#[cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
impl AsFd for OwnedFd {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        // SAFETY: `OwnedFd` and `BorrowedFd` have the same validity
        // invariants, and the `BorrowedFd` is bounded by the lifetime
        // of `&self`.
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}
