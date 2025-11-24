//! The [`pidfd_getfd`] function and supporting types.

#![allow(unsafe_code)]
use crate::fd::OwnedFd;
use crate::{backend, ffi, io};
use backend::fd::{AsFd, RawFd};

/// Raw file descriptor in another process.
///
/// A distinct type alias is used here to inform the user that normal file
/// descriptors from the calling process should not be used. The provided file
/// descriptor is used by the kernel as the index into the file descriptor
/// table of an entirely different process.
pub type ForeignRawFd = RawFd;

bitflags::bitflags! {
    /// All flags are reserved for future use.
    #[repr(transparent)]
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct PidfdGetfdFlags: ffi::c_uint {
        /// <https://docs.rs/bitflags/*/bitflags/#externally-defined-flags>
        const _ = !0;
    }
}

/// `syscall(SYS_pidfd_getfd, pidfd, flags)`—Obtain a duplicate of another
/// process' file descriptor.
///
/// # References
///  - [Linux]
///
/// # Warning
///
/// This function is generally safe for the calling process, but it can impact
/// the target process in unexpected ways. If you want to ensure that Rust I/O
/// safety assumptions continue to hold in the target process, then the target
/// process must have communicated the file description number to the calling
/// process from a value of a type that implements `AsRawFd`, and the target
/// process must not drop that value until after the calling process has
/// returned from `pidfd_getfd`.
///
/// When `pidfd_getfd` is used to debug the target, or the target is not a Rust
/// application, or `pidfd_getfd` is used in any other way, then extra care
/// should be taken to avoid unexpected behaviour or crashes.
///
/// For further details, see the references above.
///
/// [Linux]: https://man7.org/linux/man-pages/man2/pidfd_getfd.2.html
#[inline]
pub fn pidfd_getfd<Fd: AsFd>(
    pidfd: Fd,
    targetfd: ForeignRawFd,
    flags: PidfdGetfdFlags,
) -> io::Result<OwnedFd> {
    backend::process::syscalls::pidfd_getfd(pidfd.as_fd(), targetfd, flags)
}
