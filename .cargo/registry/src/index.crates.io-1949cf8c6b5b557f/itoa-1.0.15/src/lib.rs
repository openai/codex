//! [![github]](https://github.com/dtolnay/itoa)&ensp;[![crates-io]](https://crates.io/crates/itoa)&ensp;[![docs-rs]](https://docs.rs/itoa)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! This crate provides a fast conversion of integer primitives to decimal
//! strings. The implementation comes straight from [libcore] but avoids the
//! performance penalty of going through [`core::fmt::Formatter`].
//!
//! See also [`ryu`] for printing floating point primitives.
//!
//! [libcore]: https://github.com/rust-lang/rust/blob/b8214dc6c6fc20d0a660fb5700dca9ebf51ebe89/src/libcore/fmt/num.rs#L201-L254
//! [`ryu`]: https://github.com/dtolnay/ryu
//!
//! # Example
//!
//! ```
//! fn main() {
//!     let mut buffer = itoa::Buffer::new();
//!     let printed = buffer.format(128u64);
//!     assert_eq!(printed, "128");
//! }
//! ```
//!
//! # Performance (lower is better)
//!
//! ![performance](https://raw.githubusercontent.com/dtolnay/itoa/master/performance.png)

#![doc(html_root_url = "https://docs.rs/itoa/1.0.15")]
#![no_std]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::expl_impl_clone_on_copy,
    clippy::must_use_candidate,
    clippy::needless_doctest_main,
    clippy::unreadable_literal
)]

mod udiv128;

use core::hint;
use core::mem::MaybeUninit;
use core::{ptr, slice, str};
#[cfg(feature = "no-panic")]
use no_panic::no_panic;

/// A correctly sized stack allocation for the formatted integer to be written
/// into.
///
/// # Example
///
/// ```
/// let mut buffer = itoa::Buffer::new();
/// let printed = buffer.format(1234);
/// assert_eq!(printed, "1234");
/// ```
pub struct Buffer {
    bytes: [MaybeUninit<u8>; i128::MAX_STR_LEN],
}

impl Default for Buffer {
    #[inline]
    fn default() -> Buffer {
        Buffer::new()
    }
}

impl Copy for Buffer {}

impl Clone for Buffer {
    #[inline]
    #[allow(clippy::non_canonical_clone_impl)] // false positive https://github.com/rust-lang/rust-clippy/issues/11072
    fn clone(&self) -> Self {
        Buffer::new()
    }
}

impl Buffer {
    /// This is a cheap operation; you don't need to worry about reusing buffers
    /// for efficiency.
    #[inline]
    #[cfg_attr(feature = "no-panic", no_panic)]
    pub fn new() -> Buffer {
        let bytes = [MaybeUninit::<u8>::uninit(); i128::MAX_STR_LEN];
        Buffer { bytes }
    }

    /// Print an integer into this buffer and return a reference to its string
    /// representation within the buffer.
    #[cfg_attr(feature = "no-panic", no_panic)]
    pub fn format<I: Integer>(&mut self, i: I) -> &str {
        let string = i.write(unsafe {
            &mut *(&mut self.bytes as *mut [MaybeUninit<u8>; i128::MAX_STR_LEN]
                as *mut <I as private::Sealed>::Buffer)
        });
        if string.len() > I::MAX_STR_LEN {
            unsafe { hint::unreachable_unchecked() };
        }
        string
    }
}

/// An integer that can be written into an [`itoa::Buffer`][Buffer].
///
/// This trait is sealed and cannot be implemented for types outside of itoa.
pub trait Integer: private::Sealed {
    /// The maximum length of string that formatting an integer of this type can
    /// produce on the current target platform.
    const MAX_STR_LEN: usize;
}

// Seal to prevent downstream implementations of the Integer trait.
mod private {
    #[doc(hidden)]
    pub trait Sealed: Copy {
        #[doc(hidden)]
        type Buffer: 'static;
        fn write(self, buf: &mut Self::Buffer) -> &str;
    }
}

const DEC_DIGITS_LUT: [u8; 200] = *b"\
      0001020304050607080910111213141516171819\
      2021222324252627282930313233343536373839\
      4041424344454647484950515253545556575859\
      6061626364656667686970717273747576777879\
      8081828384858687888990919293949596979899";

// Adaptation of the original implementation at
// https://github.com/rust-lang/rust/blob/b8214dc6c6fc20d0a660fb5700dca9ebf51ebe89/src/libcore/fmt/num.rs#L188-L266
macro_rules! impl_Integer {
    ($t:ty[len = $max_len:expr] as $large_unsigned:ty) => {
        impl Integer for $t {
            const MAX_STR_LEN: usize = $max_len;
        }

        impl private::Sealed for $t {
            type Buffer = [MaybeUninit<u8>; $max_len];

            #[allow(unused_comparisons)]
            #[inline]
            #[cfg_attr(feature = "no-panic", no_panic)]
            fn write(self, buf: &mut [MaybeUninit<u8>; $max_len]) -> &str {
                let is_nonnegative = self >= 0;
                let mut n = if is_nonnegative {
                    self as $large_unsigned
                } else {
                    // Convert negative number to positive by summing 1 to its two's complement.
                    (!(self as $large_unsigned)).wrapping_add(1)
                };
                let mut curr = buf.len();
                let buf_ptr = buf.as_mut_ptr() as *mut u8;
                let lut_ptr = DEC_DIGITS_LUT.as_ptr();

                // Render 4 digits at a time.
                while n >= 10000 {
                    let rem = n % 10000;
                    n /= 10000;

                    let d1 = ((rem / 100) << 1) as usize;
                    let d2 = ((rem % 100) << 1) as usize;
                    curr -= 4;
                    unsafe {
                        ptr::copy_nonoverlapping(lut_ptr.add(d1), buf_ptr.add(curr), 2);
                        ptr::copy_nonoverlapping(lut_ptr.add(d2), buf_ptr.add(curr + 2), 2);
                    }
                }

                // Render 2 more digits, if >2 digits.
                if n >= 100 {
                    let d1 = ((n % 100) << 1) as usize;
                    n /= 100;
                    curr -= 2;
                    unsafe {
                        ptr::copy_nonoverlapping(lut_ptr.add(d1), buf_ptr.add(curr), 2);
                    }
                }

                // Render last 1 or 2 digits.
                if n < 10 {
                    curr -= 1;
                    unsafe {
                        *buf_ptr.add(curr) = (n as u8) + b'0';
                    }
                } else {
                    let d1 = (n << 1) as usize;
                    curr -= 2;
                    unsafe {
                        ptr::copy_nonoverlapping(lut_ptr.add(d1), buf_ptr.add(curr), 2);
                    }
                }

                if !is_nonnegative {
                    curr -= 1;
                    unsafe {
                        *buf_ptr.add(curr) = b'-';
                    }
                }

                let len = buf.len() - curr;
                let bytes = unsafe { slice::from_raw_parts(buf_ptr.add(curr), len) };
                unsafe { str::from_utf8_unchecked(bytes) }
            }
        }
    };
}

impl_Integer!(i8[len = 4] as u32);
impl_Integer!(u8[len = 3] as u32);
impl_Integer!(i16[len = 6] as u32);
impl_Integer!(u16[len = 5] as u32);
impl_Integer!(i32[len = 11] as u32);
impl_Integer!(u32[len = 10] as u32);
impl_Integer!(i64[len = 20] as u64);
impl_Integer!(u64[len = 20] as u64);

macro_rules! impl_Integer_size {
    ($t:ty as $primitive:ident #[cfg(target_pointer_width = $width:literal)]) => {
        #[cfg(target_pointer_width = $width)]
        impl Integer for $t {
            const MAX_STR_LEN: usize = <$primitive as Integer>::MAX_STR_LEN;
        }

        #[cfg(target_pointer_width = $width)]
        impl private::Sealed for $t {
            type Buffer = <$primitive as private::Sealed>::Buffer;

            #[inline]
            #[cfg_attr(feature = "no-panic", no_panic)]
            fn write(self, buf: &mut Self::Buffer) -> &str {
                (self as $primitive).write(buf)
            }
        }
    };
}

impl_Integer_size!(isize as i16 #[cfg(target_pointer_width = "16")]);
impl_Integer_size!(usize as u16 #[cfg(target_pointer_width = "16")]);
impl_Integer_size!(isize as i32 #[cfg(target_pointer_width = "32")]);
impl_Integer_size!(usize as u32 #[cfg(target_pointer_width = "32")]);
impl_Integer_size!(isize as i64 #[cfg(target_pointer_width = "64")]);
impl_Integer_size!(usize as u64 #[cfg(target_pointer_width = "64")]);

macro_rules! impl_Integer128 {
    ($t:ty[len = $max_len:expr]) => {
        impl Integer for $t {
            const MAX_STR_LEN: usize = $max_len;
        }

        impl private::Sealed for $t {
            type Buffer = [MaybeUninit<u8>; $max_len];

            #[allow(unused_comparisons)]
            #[inline]
            #[cfg_attr(feature = "no-panic", no_panic)]
            fn write(self, buf: &mut [MaybeUninit<u8>; $max_len]) -> &str {
                let is_nonnegative = self >= 0;
                let n = if is_nonnegative {
                    self as u128
                } else {
                    // Convert negative number to positive by summing 1 to its two's complement.
                    (!(self as u128)).wrapping_add(1)
                };
                let mut curr = buf.len();
                let buf_ptr = buf.as_mut_ptr() as *mut u8;

                // Divide by 10^19 which is the highest power less than 2^64.
                let (n, rem) = udiv128::udivmod_1e19(n);
                let buf1 = unsafe {
                    buf_ptr.add(curr - u64::MAX_STR_LEN) as *mut [MaybeUninit<u8>; u64::MAX_STR_LEN]
                };
                curr -= rem.write(unsafe { &mut *buf1 }).len();

                if n != 0 {
                    // Memset the base10 leading zeros of rem.
                    let target = buf.len() - 19;
                    unsafe {
                        ptr::write_bytes(buf_ptr.add(target), b'0', curr - target);
                    }
                    curr = target;

                    // Divide by 10^19 again.
                    let (n, rem) = udiv128::udivmod_1e19(n);
                    let buf2 = unsafe {
                        buf_ptr.add(curr - u64::MAX_STR_LEN)
                            as *mut [MaybeUninit<u8>; u64::MAX_STR_LEN]
                    };
                    curr -= rem.write(unsafe { &mut *buf2 }).len();

                    if n != 0 {
                        // Memset the leading zeros.
                        let target = buf.len() - 38;
                        unsafe {
                            ptr::write_bytes(buf_ptr.add(target), b'0', curr - target);
                        }
                        curr = target;

                        // There is at most one digit left
                        // because u128::MAX / 10^19 / 10^19 is 3.
                        curr -= 1;
                        unsafe {
                            *buf_ptr.add(curr) = (n as u8) + b'0';
                        }
                    }
                }

                if !is_nonnegative {
                    curr -= 1;
                    unsafe {
                        *buf_ptr.add(curr) = b'-';
                    }
                }

                let len = buf.len() - curr;
                let bytes = unsafe { slice::from_raw_parts(buf_ptr.add(curr), len) };
                unsafe { str::from_utf8_unchecked(bytes) }
            }
        }
    };
}

impl_Integer128!(i128[len = 40]);
impl_Integer128!(u128[len = 39]);
