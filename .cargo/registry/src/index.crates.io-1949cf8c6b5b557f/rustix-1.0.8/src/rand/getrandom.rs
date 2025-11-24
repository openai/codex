//! Wrappers for `getrandom`.

#![allow(unsafe_code)]

use crate::buffer::Buffer;
use crate::{backend, io};

pub use backend::rand::types::GetRandomFlags;

/// `getrandom(buf, flags)`—Reads a sequence of random bytes.
///
/// This is a very low-level API which may be difficult to use correctly. Most
/// users should prefer to use [`getrandom`] or [`rand`] APIs instead.
///
/// This function is implemented using a system call, and not the
/// [vDSO mechanism] introduced in Linux 6.11. See [#1185] for details.
///
/// [`getrandom`]: https://crates.io/crates/getrandom
/// [`rand`]: https://crates.io/crates/rand
/// [vDSO mechanism]: https://lwn.net/Articles/983186/
/// [#1185]: https://github.com/bytecodealliance/rustix/issues/1185
///
/// # References
///  - [Linux]
///
/// [Linux]: https://man7.org/linux/man-pages/man2/getrandom.2.html
#[inline]
pub fn getrandom<Buf: Buffer<u8>>(mut buf: Buf, flags: GetRandomFlags) -> io::Result<Buf::Output> {
    // SAFETY: `getrandom` behaves.
    let len = unsafe { backend::rand::syscalls::getrandom(buf.parts_mut(), flags)? };
    // SAFETY: `getrandom` behaves.
    unsafe { Ok(buf.assume_init(len)) }
}
