#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! Provides abstractions for working with bytes.
//!
//! The `bytes` crate provides an efficient byte buffer structure
//! ([`Bytes`]) and traits for working with buffer
//! implementations ([`Buf`], [`BufMut`]).
//!
//! # `Bytes`
//!
//! `Bytes` is an efficient container for storing and operating on contiguous
//! slices of memory. It is intended for use primarily in networking code, but
//! could have applications elsewhere as well.
//!
//! `Bytes` values facilitate zero-copy network programming by allowing multiple
//! `Bytes` objects to point to the same underlying memory. This is managed by
//! using a reference count to track when the memory is no longer needed and can
//! be freed.
//!
//! A `Bytes` handle can be created directly from an existing byte store (such as `&[u8]`
//! or `Vec<u8>`), but usually a `BytesMut` is used first and written to. For
//! example:
//!
//! ```rust
//! use bytes::{BytesMut, BufMut};
//!
//! let mut buf = BytesMut::with_capacity(1024);
//! buf.put(&b"hello world"[..]);
//! buf.put_u16(1234);
//!
//! let a = buf.split();
//! assert_eq!(a, b"hello world\x04\xD2"[..]);
//!
//! buf.put(&b"goodbye world"[..]);
//!
//! let b = buf.split();
//! assert_eq!(b, b"goodbye world"[..]);
//!
//! assert_eq!(buf.capacity(), 998);
//! ```
//!
//! In the above example, only a single buffer of 1024 is allocated. The handles
//! `a` and `b` will share the underlying buffer and maintain indices tracking
//! the view into the buffer represented by the handle.
//!
//! See the [struct docs](`Bytes`) for more details.
//!
//! # `Buf`, `BufMut`
//!
//! These two traits provide read and write access to buffers. The underlying
//! storage may or may not be in contiguous memory. For example, `Bytes` is a
//! buffer that guarantees contiguous memory, but a [rope] stores the bytes in
//! disjoint chunks. `Buf` and `BufMut` maintain cursors tracking the current
//! position in the underlying byte storage. When bytes are read or written, the
//! cursor is advanced.
//!
//! [rope]: https://en.wikipedia.org/wiki/Rope_(data_structure)
//!
//! ## Relation with `Read` and `Write`
//!
//! At first glance, it may seem that `Buf` and `BufMut` overlap in
//! functionality with [`std::io::Read`] and [`std::io::Write`]. However, they
//! serve different purposes. A buffer is the value that is provided as an
//! argument to `Read::read` and `Write::write`. `Read` and `Write` may then
//! perform a syscall, which has the potential of failing. Operations on `Buf`
//! and `BufMut` are infallible.

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod buf;
pub use crate::buf::{Buf, BufMut};

mod bytes;
mod bytes_mut;
mod fmt;
mod loom;
pub use crate::bytes::Bytes;
pub use crate::bytes_mut::BytesMut;

// Optional Serde support
#[cfg(feature = "serde")]
mod serde;

#[inline(never)]
#[cold]
fn abort() -> ! {
    #[cfg(feature = "std")]
    {
        std::process::abort();
    }

    #[cfg(not(feature = "std"))]
    {
        struct Abort;
        impl Drop for Abort {
            fn drop(&mut self) {
                panic!();
            }
        }
        let _a = Abort;
        panic!("abort");
    }
}

#[inline(always)]
#[cfg(feature = "std")]
fn saturating_sub_usize_u64(a: usize, b: u64) -> usize {
    use core::convert::TryFrom;
    match usize::try_from(b) {
        Ok(b) => a.saturating_sub(b),
        Err(_) => 0,
    }
}

#[inline(always)]
#[cfg(feature = "std")]
fn min_u64_usize(a: u64, b: usize) -> usize {
    use core::convert::TryFrom;
    match usize::try_from(a) {
        Ok(a) => usize::min(a, b),
        Err(_) => b,
    }
}

/// Error type for the `try_get_` methods of [`Buf`].
/// Indicates that there were not enough remaining
/// bytes in the buffer while attempting
/// to get a value from a [`Buf`] with one
/// of the `try_get_` methods.
#[derive(Debug, PartialEq, Eq)]
pub struct TryGetError {
    /// The number of bytes necessary to get the value
    pub requested: usize,

    /// The number of bytes available in the buffer
    pub available: usize,
}

impl core::fmt::Display for TryGetError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        write!(
            f,
            "Not enough bytes remaining in buffer to read value (requested {} but only {} available)",
            self.requested,
            self.available
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TryGetError {}

#[cfg(feature = "std")]
impl From<TryGetError> for std::io::Error {
    fn from(error: TryGetError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, error)
    }
}

/// Panic with a nice error message.
#[cold]
fn panic_advance(error_info: &TryGetError) -> ! {
    panic!(
        "advance out of bounds: the len is {} but advancing by {}",
        error_info.available, error_info.requested
    );
}

#[cold]
fn panic_does_not_fit(size: usize, nbytes: usize) -> ! {
    panic!(
        "size too large: the integer type can fit {} bytes, but nbytes is {}",
        size, nbytes
    );
}

/// Precondition: dst >= original
///
/// The following line is equivalent to:
///
/// ```rust,ignore
/// self.ptr.as_ptr().offset_from(ptr) as usize;
/// ```
///
/// But due to min rust is 1.39 and it is only stabilized
/// in 1.47, we cannot use it.
#[inline]
fn offset_from(dst: *const u8, original: *const u8) -> usize {
    dst as usize - original as usize
}
