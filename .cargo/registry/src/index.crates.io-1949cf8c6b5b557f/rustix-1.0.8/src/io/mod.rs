//! I/O operations.
//!
//! If you're looking for [`SeekFrom`], it's in the [`fs`] module.
//!
//! [`SeekFrom`]: crate::fs::SeekFrom
//! [`fs`]: crate::fs

mod close;
#[cfg(not(windows))]
mod dup;
mod errno;
#[cfg(not(windows))]
mod fcntl;
mod ioctl;
mod read_write;

pub use close::*;
#[cfg(not(windows))]
pub use dup::*;
pub use errno::{retry_on_intr, Errno, Result};
#[cfg(not(windows))]
pub use fcntl::*;
pub use ioctl::*;
pub use read_write::*;
