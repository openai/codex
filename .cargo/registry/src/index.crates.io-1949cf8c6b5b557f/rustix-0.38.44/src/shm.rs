//! POSIX shared memory
//!
//! # Example
//!
//! ```
//! use rustix::fs::{ftruncate, Mode};
//! use rustix::mm::{mmap, MapFlags, ProtFlags};
//! use rustix::{io, shm};
//! use std::mem::size_of;
//! use std::ptr::null_mut;
//!
//! # fn example() -> io::Result<()> {
//! // A type describing the data to be shared.
//! #[repr(C)]
//! struct MyBufferType {
//!     // …
//! }
//!
//! // Create the shared memory object.
//! let shm_path = "/rustix-shm-example";
//! let fd = shm::open(
//!     shm_path,
//!     shm::OFlags::CREATE | shm::OFlags::EXCL | shm::OFlags::RDWR,
//!     Mode::RUSR | Mode::WUSR,
//! )?;
//!
//! // Resize the shared memory object to the size of our data.
//! ftruncate(&fd, size_of::<MyBufferType>() as u64)?;
//!
//! // Map the shared memory object into our address space.
//! //
//! // SAFETY: We're creating a new mapping that's independent of any existing
//! // memory allocations. There are interesting things to say about *using*
//! // `ptr`, but that's for another safety comment.
//! let ptr = unsafe {
//!     mmap(
//!         null_mut(),
//!         size_of::<MyBufferType>(),
//!         ProtFlags::READ | ProtFlags::WRITE,
//!         MapFlags::SHARED,
//!         &fd,
//!         0,
//!     )?
//! };
//!
//! // Use `ptr`…
//!
//! // Remove the shared memory object name.
//! shm::unlink(shm_path)?;
//! # Ok(())
//! # }
//! ```

#![allow(unused_qualifications)]

use crate::fd::OwnedFd;
use crate::{backend, io, path};

use super::shm;
pub use crate::backend::fs::types::Mode;
pub use crate::backend::shm::types::ShmOFlags as OFlags;
#[deprecated(note = "Use `shm::OFlags`.")]
#[doc(hidden)]
pub use crate::backend::shm::types::ShmOFlags;
#[deprecated(note = "Use `shm::open`.")]
#[doc(hidden)]
pub use open as shm_open;
#[deprecated(note = "Use `shm::unlink`.")]
#[doc(hidden)]
pub use unlink as shm_unlink;

/// `shm_open(name, oflags, mode)`—Opens a shared memory object.
///
/// For portability, `name` should begin with a slash, contain no other
/// slashes, and be no longer than an implementation-defined limit (255 on
/// Linux).
///
/// Exactly one of [`shm::OFlags::RDONLY`] and [`shm::OFlags::RDWR`] should be
/// passed. The file descriptor will be opened with `FD_CLOEXEC` set.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/shm_open.html
/// [Linux]: https://man7.org/linux/man-pages/man3/shm_open.3.html
#[doc(alias = "shm_open")]
#[inline]
pub fn open<P: path::Arg>(name: P, flags: shm::OFlags, mode: Mode) -> io::Result<OwnedFd> {
    name.into_with_c_str(|name| backend::shm::syscalls::shm_open(name, flags, mode))
}

/// `shm_unlink(name)`—Unlinks a shared memory object.
///
/// # References
///  - [POSIX]
///  - [Linux]
///
/// [POSIX]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/shm_unlink.html
/// [Linux]: https://man7.org/linux/man-pages/man3/shm_unlink.3.html
#[doc(alias = "shm_unlink")]
#[inline]
pub fn unlink<P: path::Arg>(name: P) -> io::Result<()> {
    name.into_with_c_str(backend::shm::syscalls::shm_unlink)
}
