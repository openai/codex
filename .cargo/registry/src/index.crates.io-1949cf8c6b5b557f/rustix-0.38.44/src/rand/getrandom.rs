//! Wrappers for `getrandom`.

#![allow(unsafe_code)]

use crate::buffer::split_init;
use crate::{backend, io};
use core::mem::MaybeUninit;

pub use backend::rand::types::GetRandomFlags;

/// `getrandom(buf, flags)`—Reads a sequence of random bytes.
///
/// This is a very low-level API which may be difficult to use correctly. Most
/// users should prefer to use [`getrandom`] or [`rand`] APIs instead.
///
/// This takes a `&mut [u8]` which Rust requires to contain initialized memory.
/// To use an uninitialized buffer, use [`getrandom_uninit`].
///
/// [`getrandom`]: https://crates.io/crates/getrandom
/// [`rand`]: https://crates.io/crates/rand
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/getrandom.2.html
#[inline]
pub fn getrandom(buf: &mut [u8], flags: GetRandomFlags) -> io::Result<usize> {
    unsafe { backend::rand::syscalls::getrandom(buf.as_mut_ptr(), buf.len(), flags) }
}

/// `getrandom(buf, flags)`—Reads a sequence of random bytes.
///
/// This is identical to [`getrandom`], except that it can read into
/// uninitialized memory. It returns the slice that was initialized by this
/// function and the slice that remains uninitialized.
#[inline]
pub fn getrandom_uninit(
    buf: &mut [MaybeUninit<u8>],
    flags: GetRandomFlags,
) -> io::Result<(&mut [u8], &mut [MaybeUninit<u8>])> {
    // Get number of initialized bytes.
    let length = unsafe {
        backend::rand::syscalls::getrandom(buf.as_mut_ptr().cast::<u8>(), buf.len(), flags)
    };

    // Split into the initialized and uninitialized portions.
    Ok(unsafe { split_init(buf, length?) })
}
