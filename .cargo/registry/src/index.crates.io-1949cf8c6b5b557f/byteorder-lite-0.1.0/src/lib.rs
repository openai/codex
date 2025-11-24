/*!

This crate is a fork of the [`byteorder`] crate which sets
`#![forbid(unsafe_code)]`. It includes all traits and most methods from the
original crate, but the `ReadBytesExt::read_*_into` family of methods had to be
removed because they currently cannot be implemented without unsafe code.

The organization of the crate is pretty simple. A trait, [`ByteOrder`], specifies
byte conversion methods for each type of number in Rust (sans numbers that have
a platform dependent size like `usize` and `isize`). Two types, [`BigEndian`]
and [`LittleEndian`] implement these methods. Finally, [`ReadBytesExt`] and
[`WriteBytesExt`] provide convenience methods available to all types that
implement [`Read`] and [`Write`].

An alias, [`NetworkEndian`], for [`BigEndian`] is provided to help improve
code clarity.

An additional alias, [`NativeEndian`], is provided for the endianness of the
local platform. This is convenient when serializing data for use and
conversions are not desired.

# Examples

Read unsigned 16 bit big-endian integers from a [`Read`] type:

```rust
use std::io::Cursor;
use byteorder_lite::{BigEndian, ReadBytesExt};

let mut rdr = Cursor::new(vec![2, 5, 3, 0]);
// Note that we use type parameters to indicate which kind of byte order
// we want!
assert_eq!(517, rdr.read_u16::<BigEndian>().unwrap());
assert_eq!(768, rdr.read_u16::<BigEndian>().unwrap());
```

Write unsigned 16 bit little-endian integers to a [`Write`] type:

```rust
use byteorder_lite::{LittleEndian, WriteBytesExt};

let mut wtr = vec![];
wtr.write_u16::<LittleEndian>(517).unwrap();
wtr.write_u16::<LittleEndian>(768).unwrap();
assert_eq!(wtr, vec![5, 2, 0, 3]);
```

# Optional Features

This crate can also be used without the standard library.

# Alternatives

The standard numeric types provide built-in methods like `to_le_bytes` and
`from_le_bytes`, which support some of the same use cases.

[`byteorder`]: https://crates.io/crates/byteorder
[`ByteOrder`]: trait.ByteOrder.html
[`BigEndian`]: enum.BigEndian.html
[`LittleEndian`]: enum.LittleEndian.html
[`ReadBytesExt`]: trait.ReadBytesExt.html
[`WriteBytesExt`]: trait.WriteBytesExt.html
[`NetworkEndian`]: type.NetworkEndian.html
[`NativeEndian`]: type.NativeEndian.html
[`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
[`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
*/

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
// When testing under miri, we disable tests that take too long. But this
// provokes lots of dead code warnings. So we just squash them.
#![cfg_attr(miri, allow(dead_code, unused_macros))]

use core::{fmt::Debug, hash::Hash};

#[cfg(feature = "std")]
pub use crate::io::{ReadBytesExt, WriteBytesExt};

#[cfg(feature = "std")]
mod io;

#[inline]
fn extend_sign(val: u64, nbytes: usize) -> i64 {
    let shift = (8 - nbytes) * 8;
    (val << shift) as i64 >> shift
}

#[inline]
fn extend_sign128(val: u128, nbytes: usize) -> i128 {
    let shift = (16 - nbytes) * 8;
    (val << shift) as i128 >> shift
}

#[inline]
fn unextend_sign(val: i64, nbytes: usize) -> u64 {
    let shift = (8 - nbytes) * 8;
    (val << shift) as u64 >> shift
}

#[inline]
fn unextend_sign128(val: i128, nbytes: usize) -> u128 {
    let shift = (16 - nbytes) * 8;
    (val << shift) as u128 >> shift
}

#[inline]
fn pack_size(n: u64) -> usize {
    (8 - ((n | 1).leading_zeros() >> 3)) as usize
}

#[inline]
fn pack_size128(n: u128) -> usize {
    (16 - ((n | 1).leading_zeros() >> 3)) as usize
}

mod private {
    /// Sealed stops crates other than byteorder from implementing any traits
    /// that use it.
    pub trait Sealed {}
    impl Sealed for super::LittleEndian {}
    impl Sealed for super::BigEndian {}
}

/// `ByteOrder` describes types that can serialize integers as bytes.
///
/// Note that `Self` does not appear anywhere in this trait's definition!
/// Therefore, in order to use it, you'll need to use syntax like
/// `T::read_u16(&[0, 1])` where `T` implements `ByteOrder`.
///
/// This crate provides two types that implement `ByteOrder`: [`BigEndian`]
/// and [`LittleEndian`].
/// This trait is sealed and cannot be implemented for callers to avoid
/// breaking backwards compatibility when adding new derived traits.
///
/// # Examples
///
/// Write and read `u32` numbers in little endian order:
///
/// ```rust
/// use byteorder_lite::{ByteOrder, LittleEndian};
///
/// let mut buf = [0; 4];
/// LittleEndian::write_u32(&mut buf, 1_000_000);
/// assert_eq!(1_000_000, LittleEndian::read_u32(&buf));
/// ```
///
/// Write and read `i16` numbers in big endian order:
///
/// ```rust
/// use byteorder_lite::{ByteOrder, BigEndian};
///
/// let mut buf = [0; 2];
/// BigEndian::write_i16(&mut buf, -5_000);
/// assert_eq!(-5_000, BigEndian::read_i16(&buf));
/// ```
///
/// [`BigEndian`]: enum.BigEndian.html
/// [`LittleEndian`]: enum.LittleEndian.html
pub trait ByteOrder:
    Clone
    + Copy
    + Debug
    + Default
    + Eq
    + Hash
    + Ord
    + PartialEq
    + PartialOrd
    + private::Sealed
{
    /// Reads an unsigned 16 bit integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 2`.
    fn read_u16(buf: &[u8]) -> u16;

    /// Reads an unsigned 24 bit integer from `buf`, stored in u32.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 3`.
    ///
    /// # Examples
    ///
    /// Write and read 24 bit `u32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_u24(&mut buf, 1_000_000);
    /// assert_eq!(1_000_000, LittleEndian::read_u24(&buf));
    /// ```
    fn read_u24(buf: &[u8]) -> u32 {
        Self::read_uint(buf, 3) as u32
    }

    /// Reads an unsigned 32 bit integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 4`.
    ///
    /// # Examples
    ///
    /// Write and read `u32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 4];
    /// LittleEndian::write_u32(&mut buf, 1_000_000);
    /// assert_eq!(1_000_000, LittleEndian::read_u32(&buf));
    /// ```
    fn read_u32(buf: &[u8]) -> u32;

    /// Reads an unsigned 48 bit integer from `buf`, stored in u64.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 6`.
    ///
    /// # Examples
    ///
    /// Write and read 48 bit `u64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 6];
    /// LittleEndian::write_u48(&mut buf, 1_000_000_000_000);
    /// assert_eq!(1_000_000_000_000, LittleEndian::read_u48(&buf));
    /// ```
    fn read_u48(buf: &[u8]) -> u64 {
        Self::read_uint(buf, 6) as u64
    }

    /// Reads an unsigned 64 bit integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 8`.
    ///
    /// # Examples
    ///
    /// Write and read `u64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 8];
    /// LittleEndian::write_u64(&mut buf, 1_000_000);
    /// assert_eq!(1_000_000, LittleEndian::read_u64(&buf));
    /// ```
    fn read_u64(buf: &[u8]) -> u64;

    /// Reads an unsigned 128 bit integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 16`.
    ///
    /// # Examples
    ///
    /// Write and read `u128` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 16];
    /// LittleEndian::write_u128(&mut buf, 1_000_000);
    /// assert_eq!(1_000_000, LittleEndian::read_u128(&buf));
    /// ```
    fn read_u128(buf: &[u8]) -> u128;

    /// Reads an unsigned n-bytes integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `nbytes < 1` or `nbytes > 8` or
    /// `buf.len() < nbytes`
    ///
    /// # Examples
    ///
    /// Write and read an n-byte number in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_uint(&mut buf, 1_000_000, 3);
    /// assert_eq!(1_000_000, LittleEndian::read_uint(&buf, 3));
    /// ```
    fn read_uint(buf: &[u8], nbytes: usize) -> u64;

    /// Reads an unsigned n-bytes integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `nbytes < 1` or `nbytes > 16` or
    /// `buf.len() < nbytes`
    ///
    /// # Examples
    ///
    /// Write and read an n-byte number in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_uint128(&mut buf, 1_000_000, 3);
    /// assert_eq!(1_000_000, LittleEndian::read_uint128(&buf, 3));
    /// ```
    fn read_uint128(buf: &[u8], nbytes: usize) -> u128;

    /// Writes an unsigned 16 bit integer `n` to `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 2`.
    ///
    /// # Examples
    ///
    /// Write and read `u16` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 2];
    /// LittleEndian::write_u16(&mut buf, 1_000);
    /// assert_eq!(1_000, LittleEndian::read_u16(&buf));
    /// ```
    fn write_u16(buf: &mut [u8], n: u16);

    /// Writes an unsigned 24 bit integer `n` to `buf`, stored in u32.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 3`.
    ///
    /// # Examples
    ///
    /// Write and read 24 bit `u32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_u24(&mut buf, 1_000_000);
    /// assert_eq!(1_000_000, LittleEndian::read_u24(&buf));
    /// ```
    fn write_u24(buf: &mut [u8], n: u32) {
        Self::write_uint(buf, n as u64, 3)
    }

    /// Writes an unsigned 32 bit integer `n` to `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 4`.
    ///
    /// # Examples
    ///
    /// Write and read `u32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 4];
    /// LittleEndian::write_u32(&mut buf, 1_000_000);
    /// assert_eq!(1_000_000, LittleEndian::read_u32(&buf));
    /// ```
    fn write_u32(buf: &mut [u8], n: u32);

    /// Writes an unsigned 48 bit integer `n` to `buf`, stored in u64.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 6`.
    ///
    /// # Examples
    ///
    /// Write and read 48 bit `u64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 6];
    /// LittleEndian::write_u48(&mut buf, 1_000_000_000_000);
    /// assert_eq!(1_000_000_000_000, LittleEndian::read_u48(&buf));
    /// ```
    fn write_u48(buf: &mut [u8], n: u64) {
        Self::write_uint(buf, n as u64, 6)
    }

    /// Writes an unsigned 64 bit integer `n` to `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 8`.
    ///
    /// # Examples
    ///
    /// Write and read `u64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 8];
    /// LittleEndian::write_u64(&mut buf, 1_000_000);
    /// assert_eq!(1_000_000, LittleEndian::read_u64(&buf));
    /// ```
    fn write_u64(buf: &mut [u8], n: u64);

    /// Writes an unsigned 128 bit integer `n` to `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 16`.
    ///
    /// # Examples
    ///
    /// Write and read `u128` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 16];
    /// LittleEndian::write_u128(&mut buf, 1_000_000);
    /// assert_eq!(1_000_000, LittleEndian::read_u128(&buf));
    /// ```
    fn write_u128(buf: &mut [u8], n: u128);

    /// Writes an unsigned integer `n` to `buf` using only `nbytes`.
    ///
    /// # Panics
    ///
    /// If `n` is not representable in `nbytes`, or if `nbytes` is `> 8`, then
    /// this method panics.
    ///
    /// # Examples
    ///
    /// Write and read an n-byte number in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_uint(&mut buf, 1_000_000, 3);
    /// assert_eq!(1_000_000, LittleEndian::read_uint(&buf, 3));
    /// ```
    fn write_uint(buf: &mut [u8], n: u64, nbytes: usize);

    /// Writes an unsigned integer `n` to `buf` using only `nbytes`.
    ///
    /// # Panics
    ///
    /// If `n` is not representable in `nbytes`, or if `nbytes` is `> 16`, then
    /// this method panics.
    ///
    /// # Examples
    ///
    /// Write and read an n-byte number in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_uint128(&mut buf, 1_000_000, 3);
    /// assert_eq!(1_000_000, LittleEndian::read_uint128(&buf, 3));
    /// ```
    fn write_uint128(buf: &mut [u8], n: u128, nbytes: usize);

    /// Reads a signed 16 bit integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 2`.
    ///
    /// # Examples
    ///
    /// Write and read `i16` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 2];
    /// LittleEndian::write_i16(&mut buf, -1_000);
    /// assert_eq!(-1_000, LittleEndian::read_i16(&buf));
    /// ```
    #[inline]
    fn read_i16(buf: &[u8]) -> i16 {
        Self::read_u16(buf) as i16
    }

    /// Reads a signed 24 bit integer from `buf`, stored in i32.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 3`.
    ///
    /// # Examples
    ///
    /// Write and read 24 bit `i32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_i24(&mut buf, -1_000_000);
    /// assert_eq!(-1_000_000, LittleEndian::read_i24(&buf));
    /// ```
    #[inline]
    fn read_i24(buf: &[u8]) -> i32 {
        Self::read_int(buf, 3) as i32
    }

    /// Reads a signed 32 bit integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 4`.
    ///
    /// # Examples
    ///
    /// Write and read `i32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 4];
    /// LittleEndian::write_i32(&mut buf, -1_000_000);
    /// assert_eq!(-1_000_000, LittleEndian::read_i32(&buf));
    /// ```
    #[inline]
    fn read_i32(buf: &[u8]) -> i32 {
        Self::read_u32(buf) as i32
    }

    /// Reads a signed 48 bit integer from `buf`, stored in i64.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 6`.
    ///
    /// # Examples
    ///
    /// Write and read 48 bit `i64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 6];
    /// LittleEndian::write_i48(&mut buf, -1_000_000_000_000);
    /// assert_eq!(-1_000_000_000_000, LittleEndian::read_i48(&buf));
    /// ```
    #[inline]
    fn read_i48(buf: &[u8]) -> i64 {
        Self::read_int(buf, 6) as i64
    }

    /// Reads a signed 64 bit integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 8`.
    ///
    /// # Examples
    ///
    /// Write and read `i64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 8];
    /// LittleEndian::write_i64(&mut buf, -1_000_000_000);
    /// assert_eq!(-1_000_000_000, LittleEndian::read_i64(&buf));
    /// ```
    #[inline]
    fn read_i64(buf: &[u8]) -> i64 {
        Self::read_u64(buf) as i64
    }

    /// Reads a signed 128 bit integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 16`.
    ///
    /// # Examples
    ///
    /// Write and read `i128` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 16];
    /// LittleEndian::write_i128(&mut buf, -1_000_000_000);
    /// assert_eq!(-1_000_000_000, LittleEndian::read_i128(&buf));
    /// ```
    #[inline]
    fn read_i128(buf: &[u8]) -> i128 {
        Self::read_u128(buf) as i128
    }

    /// Reads a signed n-bytes integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `nbytes < 1` or `nbytes > 8` or
    /// `buf.len() < nbytes`
    ///
    /// # Examples
    ///
    /// Write and read n-length signed numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_int(&mut buf, -1_000, 3);
    /// assert_eq!(-1_000, LittleEndian::read_int(&buf, 3));
    /// ```
    #[inline]
    fn read_int(buf: &[u8], nbytes: usize) -> i64 {
        extend_sign(Self::read_uint(buf, nbytes), nbytes)
    }

    /// Reads a signed n-bytes integer from `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `nbytes < 1` or `nbytes > 16` or
    /// `buf.len() < nbytes`
    ///
    /// # Examples
    ///
    /// Write and read n-length signed numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_int128(&mut buf, -1_000, 3);
    /// assert_eq!(-1_000, LittleEndian::read_int128(&buf, 3));
    /// ```
    #[inline]
    fn read_int128(buf: &[u8], nbytes: usize) -> i128 {
        extend_sign128(Self::read_uint128(buf, nbytes), nbytes)
    }

    /// Reads a IEEE754 single-precision (4 bytes) floating point number.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 4`.
    ///
    /// # Examples
    ///
    /// Write and read `f32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let e = 2.71828;
    /// let mut buf = [0; 4];
    /// LittleEndian::write_f32(&mut buf, e);
    /// assert_eq!(e, LittleEndian::read_f32(&buf));
    /// ```
    #[inline]
    fn read_f32(buf: &[u8]) -> f32 {
        f32::from_bits(Self::read_u32(buf))
    }

    /// Reads a IEEE754 double-precision (8 bytes) floating point number.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 8`.
    ///
    /// # Examples
    ///
    /// Write and read `f64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let phi = 1.6180339887;
    /// let mut buf = [0; 8];
    /// LittleEndian::write_f64(&mut buf, phi);
    /// assert_eq!(phi, LittleEndian::read_f64(&buf));
    /// ```
    #[inline]
    fn read_f64(buf: &[u8]) -> f64 {
        f64::from_bits(Self::read_u64(buf))
    }

    /// Writes a signed 16 bit integer `n` to `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 2`.
    ///
    /// # Examples
    ///
    /// Write and read `i16` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 2];
    /// LittleEndian::write_i16(&mut buf, -1_000);
    /// assert_eq!(-1_000, LittleEndian::read_i16(&buf));
    /// ```
    #[inline]
    fn write_i16(buf: &mut [u8], n: i16) {
        Self::write_u16(buf, n as u16)
    }

    /// Writes a signed 24 bit integer `n` to `buf`, stored in i32.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 3`.
    ///
    /// # Examples
    ///
    /// Write and read 24 bit `i32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_i24(&mut buf, -1_000_000);
    /// assert_eq!(-1_000_000, LittleEndian::read_i24(&buf));
    /// ```
    #[inline]
    fn write_i24(buf: &mut [u8], n: i32) {
        Self::write_int(buf, n as i64, 3)
    }

    /// Writes a signed 32 bit integer `n` to `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 4`.
    ///
    /// # Examples
    ///
    /// Write and read `i32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 4];
    /// LittleEndian::write_i32(&mut buf, -1_000_000);
    /// assert_eq!(-1_000_000, LittleEndian::read_i32(&buf));
    /// ```
    #[inline]
    fn write_i32(buf: &mut [u8], n: i32) {
        Self::write_u32(buf, n as u32)
    }

    /// Writes a signed 48 bit integer `n` to `buf`, stored in i64.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 6`.
    ///
    /// # Examples
    ///
    /// Write and read 48 bit `i64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 6];
    /// LittleEndian::write_i48(&mut buf, -1_000_000_000_000);
    /// assert_eq!(-1_000_000_000_000, LittleEndian::read_i48(&buf));
    /// ```
    #[inline]
    fn write_i48(buf: &mut [u8], n: i64) {
        Self::write_int(buf, n as i64, 6)
    }

    /// Writes a signed 64 bit integer `n` to `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 8`.
    ///
    /// # Examples
    ///
    /// Write and read `i64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 8];
    /// LittleEndian::write_i64(&mut buf, -1_000_000_000);
    /// assert_eq!(-1_000_000_000, LittleEndian::read_i64(&buf));
    /// ```
    #[inline]
    fn write_i64(buf: &mut [u8], n: i64) {
        Self::write_u64(buf, n as u64)
    }

    /// Writes a signed 128 bit integer `n` to `buf`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 16`.
    ///
    /// # Examples
    ///
    /// Write and read n-byte `i128` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 16];
    /// LittleEndian::write_i128(&mut buf, -1_000_000_000);
    /// assert_eq!(-1_000_000_000, LittleEndian::read_i128(&buf));
    /// ```
    #[inline]
    fn write_i128(buf: &mut [u8], n: i128) {
        Self::write_u128(buf, n as u128)
    }

    /// Writes a signed integer `n` to `buf` using only `nbytes`.
    ///
    /// # Panics
    ///
    /// If `n` is not representable in `nbytes`, or if `nbytes` is `> 8`, then
    /// this method panics.
    ///
    /// # Examples
    ///
    /// Write and read an n-byte number in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_int(&mut buf, -1_000, 3);
    /// assert_eq!(-1_000, LittleEndian::read_int(&buf, 3));
    /// ```
    #[inline]
    fn write_int(buf: &mut [u8], n: i64, nbytes: usize) {
        Self::write_uint(buf, unextend_sign(n, nbytes), nbytes)
    }

    /// Writes a signed integer `n` to `buf` using only `nbytes`.
    ///
    /// # Panics
    ///
    /// If `n` is not representable in `nbytes`, or if `nbytes` is `> 16`, then
    /// this method panics.
    ///
    /// # Examples
    ///
    /// Write and read n-length signed numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut buf = [0; 3];
    /// LittleEndian::write_int128(&mut buf, -1_000, 3);
    /// assert_eq!(-1_000, LittleEndian::read_int128(&buf, 3));
    /// ```
    #[inline]
    fn write_int128(buf: &mut [u8], n: i128, nbytes: usize) {
        Self::write_uint128(buf, unextend_sign128(n, nbytes), nbytes)
    }

    /// Writes a IEEE754 single-precision (4 bytes) floating point number.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 4`.
    ///
    /// # Examples
    ///
    /// Write and read `f32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let e = 2.71828;
    /// let mut buf = [0; 4];
    /// LittleEndian::write_f32(&mut buf, e);
    /// assert_eq!(e, LittleEndian::read_f32(&buf));
    /// ```
    #[inline]
    fn write_f32(buf: &mut [u8], n: f32) {
        Self::write_u32(buf, n.to_bits())
    }

    /// Writes a IEEE754 double-precision (8 bytes) floating point number.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() < 8`.
    ///
    /// # Examples
    ///
    /// Write and read `f64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let phi = 1.6180339887;
    /// let mut buf = [0; 8];
    /// LittleEndian::write_f64(&mut buf, phi);
    /// assert_eq!(phi, LittleEndian::read_f64(&buf));
    /// ```
    #[inline]
    fn write_f64(buf: &mut [u8], n: f64) {
        Self::write_u64(buf, n.to_bits())
    }

    /// Reads unsigned 16 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 2*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `u16` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 8];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_u16_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_u16_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn read_u16_into(src: &[u8], dst: &mut [u16]);

    /// Reads unsigned 32 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 4*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `u32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 16];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_u32_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_u32_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn read_u32_into(src: &[u8], dst: &mut [u32]);

    /// Reads unsigned 64 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 8*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `u64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 32];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_u64_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_u64_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn read_u64_into(src: &[u8], dst: &mut [u64]);

    /// Reads unsigned 128 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 16*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `u128` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 64];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_u128_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_u128_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn read_u128_into(src: &[u8], dst: &mut [u128]);

    /// Reads signed 16 bit integers from `src` to `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() != 2*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `i16` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 8];
    /// let numbers_given = [1, 2, 0x0f, 0xee];
    /// LittleEndian::write_i16_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_i16_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn read_i16_into(src: &[u8], dst: &mut [i16]);

    /// Reads signed 32 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 4*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `i32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 16];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_i32_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_i32_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn read_i32_into(src: &[u8], dst: &mut [i32]);

    /// Reads signed 64 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 8*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `i64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 32];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_i64_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_i64_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn read_i64_into(src: &[u8], dst: &mut [i64]);

    /// Reads signed 128 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 16*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `i128` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 64];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_i128_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_i128_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn read_i128_into(src: &[u8], dst: &mut [i128]);

    /// Reads IEEE754 single-precision (4 bytes) floating point numbers from
    /// `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 4*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `f32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 16];
    /// let numbers_given = [1.0, 2.0, 31.312e31, -11.32e19];
    /// LittleEndian::write_f32_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0.0; 4];
    /// LittleEndian::read_f32_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn read_f32_into(src: &[u8], dst: &mut [f32]);

    /// **DEPRECATED**.
    ///
    /// This method is deprecated. Use `read_f32_into` instead.
    /// Reads IEEE754 single-precision (4 bytes) floating point numbers from
    /// `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 4*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `f32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 16];
    /// let numbers_given = [1.0, 2.0, 31.312e31, -11.32e19];
    /// LittleEndian::write_f32_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0.0; 4];
    /// LittleEndian::read_f32_into_unchecked(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    #[inline]
    #[deprecated(since = "1.3.0", note = "please use `read_f32_into` instead")]
    fn read_f32_into_unchecked(src: &[u8], dst: &mut [f32]) {
        Self::read_f32_into(src, dst);
    }

    /// Reads IEEE754 single-precision (4 bytes) floating point numbers from
    /// `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 8*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `f64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 32];
    /// let numbers_given = [1.0, 2.0, 31.312e211, -11.32e91];
    /// LittleEndian::write_f64_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0.0; 4];
    /// LittleEndian::read_f64_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn read_f64_into(src: &[u8], dst: &mut [f64]);

    /// **DEPRECATED**.
    ///
    /// This method is deprecated. Use `read_f64_into` instead.
    ///
    /// Reads IEEE754 single-precision (4 bytes) floating point numbers from
    /// `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 8*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `f64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 32];
    /// let numbers_given = [1.0, 2.0, 31.312e211, -11.32e91];
    /// LittleEndian::write_f64_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0.0; 4];
    /// LittleEndian::read_f64_into_unchecked(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    #[inline]
    #[deprecated(since = "1.3.0", note = "please use `read_f64_into` instead")]
    fn read_f64_into_unchecked(src: &[u8], dst: &mut [f64]) {
        Self::read_f64_into(src, dst);
    }

    /// Writes unsigned 16 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `dst.len() != 2*src.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `u16` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 8];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_u16_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_u16_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn write_u16_into(src: &[u16], dst: &mut [u8]);

    /// Writes unsigned 32 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `dst.len() != 4*src.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `u32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 16];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_u32_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_u32_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn write_u32_into(src: &[u32], dst: &mut [u8]);

    /// Writes unsigned 64 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `dst.len() != 8*src.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `u64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 32];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_u64_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_u64_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn write_u64_into(src: &[u64], dst: &mut [u8]);

    /// Writes unsigned 128 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `dst.len() != 16*src.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `u128` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 64];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_u128_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_u128_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn write_u128_into(src: &[u128], dst: &mut [u8]);

    /// Writes signed 8 bit integers from `src` into `dst`.
    ///
    /// Note that since each `i8` is a single byte, no byte order conversions
    /// are used. This method is included because it provides a safe, simple
    /// way for the caller to write from a `&[i8]` buffer. (Without this
    /// method, the caller would have to either use `unsafe` code or convert
    /// each byte to `u8` individually.)
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() != src.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `i8` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian, ReadBytesExt};
    ///
    /// let mut bytes = [0; 4];
    /// let numbers_given = [1, 2, 0xf, 0xe];
    /// LittleEndian::write_i8_into(&numbers_given, &mut bytes);
    /// ```
    fn write_i8_into(src: &[i8], dst: &mut [u8]) {
        assert_eq!(src.len(), dst.len());
        for (d, s) in dst.iter_mut().zip(src.iter()) {
            *d = *s as u8;
        }
    }

    /// Writes signed 16 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `buf.len() != 2*src.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `i16` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 8];
    /// let numbers_given = [1, 2, 0x0f, 0xee];
    /// LittleEndian::write_i16_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_i16_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn write_i16_into(src: &[i16], dst: &mut [u8]);

    /// Writes signed 32 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `dst.len() != 4*src.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `i32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 16];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_i32_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_i32_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn write_i32_into(src: &[i32], dst: &mut [u8]);

    /// Writes signed 64 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `dst.len() != 8*src.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `i64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 32];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_i64_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_i64_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn write_i64_into(src: &[i64], dst: &mut [u8]);

    /// Writes signed 128 bit integers from `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `dst.len() != 16*src.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `i128` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 64];
    /// let numbers_given = [1, 2, 0xf00f, 0xffee];
    /// LittleEndian::write_i128_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0; 4];
    /// LittleEndian::read_i128_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn write_i128_into(src: &[i128], dst: &mut [u8]);

    /// Writes IEEE754 single-precision (4 bytes) floating point numbers from
    /// `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 4*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `f32` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 16];
    /// let numbers_given = [1.0, 2.0, 31.312e31, -11.32e19];
    /// LittleEndian::write_f32_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0.0; 4];
    /// LittleEndian::read_f32_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn write_f32_into(src: &[f32], dst: &mut [u8]);

    /// Writes IEEE754 double-precision (8 bytes) floating point numbers from
    /// `src` into `dst`.
    ///
    /// # Panics
    ///
    /// Panics when `src.len() != 8*dst.len()`.
    ///
    /// # Examples
    ///
    /// Write and read `f64` numbers in little endian order:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, LittleEndian};
    ///
    /// let mut bytes = [0; 32];
    /// let numbers_given = [1.0, 2.0, 31.312e211, -11.32e91];
    /// LittleEndian::write_f64_into(&numbers_given, &mut bytes);
    ///
    /// let mut numbers_got = [0.0; 4];
    /// LittleEndian::read_f64_into(&bytes, &mut numbers_got);
    /// assert_eq!(numbers_given, numbers_got);
    /// ```
    fn write_f64_into(src: &[f64], dst: &mut [u8]);

    /// Converts the given slice of unsigned 16 bit integers to a particular
    /// endianness.
    ///
    /// If the endianness matches the endianness of the host platform, then
    /// this is a no-op.
    ///
    /// # Examples
    ///
    /// Convert the host platform's endianness to big-endian:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, BigEndian};
    ///
    /// let mut numbers = [5, 65000];
    /// BigEndian::from_slice_u16(&mut numbers);
    /// assert_eq!(numbers, [5u16.to_be(), 65000u16.to_be()]);
    /// ```
    fn from_slice_u16(numbers: &mut [u16]);

    /// Converts the given slice of unsigned 32 bit integers to a particular
    /// endianness.
    ///
    /// If the endianness matches the endianness of the host platform, then
    /// this is a no-op.
    ///
    /// # Examples
    ///
    /// Convert the host platform's endianness to big-endian:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, BigEndian};
    ///
    /// let mut numbers = [5, 65000];
    /// BigEndian::from_slice_u32(&mut numbers);
    /// assert_eq!(numbers, [5u32.to_be(), 65000u32.to_be()]);
    /// ```
    fn from_slice_u32(numbers: &mut [u32]);

    /// Converts the given slice of unsigned 64 bit integers to a particular
    /// endianness.
    ///
    /// If the endianness matches the endianness of the host platform, then
    /// this is a no-op.
    ///
    /// # Examples
    ///
    /// Convert the host platform's endianness to big-endian:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, BigEndian};
    ///
    /// let mut numbers = [5, 65000];
    /// BigEndian::from_slice_u64(&mut numbers);
    /// assert_eq!(numbers, [5u64.to_be(), 65000u64.to_be()]);
    /// ```
    fn from_slice_u64(numbers: &mut [u64]);

    /// Converts the given slice of unsigned 128 bit integers to a particular
    /// endianness.
    ///
    /// If the endianness matches the endianness of the host platform, then
    /// this is a no-op.
    ///
    /// # Examples
    ///
    /// Convert the host platform's endianness to big-endian:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, BigEndian};
    ///
    /// let mut numbers = [5, 65000];
    /// BigEndian::from_slice_u128(&mut numbers);
    /// assert_eq!(numbers, [5u128.to_be(), 65000u128.to_be()]);
    /// ```
    fn from_slice_u128(numbers: &mut [u128]);

    /// Converts the given slice of signed 16 bit integers to a particular
    /// endianness.
    ///
    /// If the endianness matches the endianness of the host platform, then
    /// this is a no-op.
    ///
    /// # Examples
    ///
    /// Convert the host platform's endianness to big-endian:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, BigEndian};
    ///
    /// let mut numbers = [5, 6500];
    /// BigEndian::from_slice_i16(&mut numbers);
    /// assert_eq!(numbers, [5i16.to_be(), 6500i16.to_be()]);
    /// ```
    fn from_slice_i16(src: &mut [i16]);

    /// Converts the given slice of signed 32 bit integers to a particular
    /// endianness.
    ///
    /// If the endianness matches the endianness of the host platform, then
    /// this is a no-op.
    ///
    /// # Examples
    ///
    /// Convert the host platform's endianness to big-endian:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, BigEndian};
    ///
    /// let mut numbers = [5, 65000];
    /// BigEndian::from_slice_i32(&mut numbers);
    /// assert_eq!(numbers, [5i32.to_be(), 65000i32.to_be()]);
    /// ```
    fn from_slice_i32(src: &mut [i32]);

    /// Converts the given slice of signed 64 bit integers to a particular
    /// endianness.
    ///
    /// If the endianness matches the endianness of the host platform, then
    /// this is a no-op.
    ///
    /// # Examples
    ///
    /// Convert the host platform's endianness to big-endian:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, BigEndian};
    ///
    /// let mut numbers = [5, 65000];
    /// BigEndian::from_slice_i64(&mut numbers);
    /// assert_eq!(numbers, [5i64.to_be(), 65000i64.to_be()]);
    /// ```
    fn from_slice_i64(src: &mut [i64]);

    /// Converts the given slice of signed 128 bit integers to a particular
    /// endianness.
    ///
    /// If the endianness matches the endianness of the host platform, then
    /// this is a no-op.
    ///
    /// # Examples
    ///
    /// Convert the host platform's endianness to big-endian:
    ///
    /// ```rust
    /// use byteorder_lite::{ByteOrder, BigEndian};
    ///
    /// let mut numbers = [5, 65000];
    /// BigEndian::from_slice_i128(&mut numbers);
    /// assert_eq!(numbers, [5i128.to_be(), 65000i128.to_be()]);
    /// ```
    fn from_slice_i128(src: &mut [i128]);

    /// Converts the given slice of IEEE754 single-precision (4 bytes) floating
    /// point numbers to a particular endianness.
    ///
    /// If the endianness matches the endianness of the host platform, then
    /// this is a no-op.
    fn from_slice_f32(numbers: &mut [f32]);

    /// Converts the given slice of IEEE754 double-precision (8 bytes) floating
    /// point numbers to a particular endianness.
    ///
    /// If the endianness matches the endianness of the host platform, then
    /// this is a no-op.
    fn from_slice_f64(numbers: &mut [f64]);
}

/// Defines big-endian serialization.
///
/// Note that this type has no value constructor. It is used purely at the
/// type level.
///
/// # Examples
///
/// Write and read `u32` numbers in big endian order:
///
/// ```rust
/// use byteorder_lite::{ByteOrder, BigEndian};
///
/// let mut buf = [0; 4];
/// BigEndian::write_u32(&mut buf, 1_000_000);
/// assert_eq!(1_000_000, BigEndian::read_u32(&buf));
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BigEndian {}

impl Default for BigEndian {
    fn default() -> BigEndian {
        panic!("BigEndian default")
    }
}

/// A type alias for [`BigEndian`].
///
/// [`BigEndian`]: enum.BigEndian.html
pub type BE = BigEndian;

/// Defines little-endian serialization.
///
/// Note that this type has no value constructor. It is used purely at the
/// type level.
///
/// # Examples
///
/// Write and read `u32` numbers in little endian order:
///
/// ```rust
/// use byteorder_lite::{ByteOrder, LittleEndian};
///
/// let mut buf = [0; 4];
/// LittleEndian::write_u32(&mut buf, 1_000_000);
/// assert_eq!(1_000_000, LittleEndian::read_u32(&buf));
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LittleEndian {}

impl Default for LittleEndian {
    fn default() -> LittleEndian {
        panic!("LittleEndian default")
    }
}

/// A type alias for [`LittleEndian`].
///
/// [`LittleEndian`]: enum.LittleEndian.html
pub type LE = LittleEndian;

/// Defines network byte order serialization.
///
/// Network byte order is defined by [RFC 1700][1] to be big-endian, and is
/// referred to in several protocol specifications.  This type is an alias of
/// [`BigEndian`].
///
/// [1]: https://tools.ietf.org/html/rfc1700
///
/// Note that this type has no value constructor. It is used purely at the
/// type level.
///
/// # Examples
///
/// Write and read `i16` numbers in big endian order:
///
/// ```rust
/// use byteorder_lite::{ByteOrder, NetworkEndian, BigEndian};
///
/// let mut buf = [0; 2];
/// BigEndian::write_i16(&mut buf, -5_000);
/// assert_eq!(-5_000, NetworkEndian::read_i16(&buf));
/// ```
///
/// [`BigEndian`]: enum.BigEndian.html
pub type NetworkEndian = BigEndian;

/// Defines system native-endian serialization.
///
/// Note that this type has no value constructor. It is used purely at the
/// type level.
///
/// On this platform, this is an alias for [`LittleEndian`].
///
/// [`LittleEndian`]: enum.LittleEndian.html
#[cfg(target_endian = "little")]
pub type NativeEndian = LittleEndian;

/// Defines system native-endian serialization.
///
/// Note that this type has no value constructor. It is used purely at the
/// type level.
///
/// On this platform, this is an alias for [`BigEndian`].
///
/// [`BigEndian`]: enum.BigEndian.html
#[cfg(target_endian = "big")]
pub type NativeEndian = BigEndian;

/// Copies a &[u8] $src into a &mut [$ty] $dst for the endianness given by
/// $from_bytes (must be either from_be_bytes or from_le_bytes).
///
/// Panics if $src.len() != $dst.len() * size_of::<$ty>().
macro_rules! read_slice {
    ($src:expr, $dst:expr, $ty:ty, $from_bytes:ident) => {{
        const SIZE: usize = core::mem::size_of::<$ty>();
        // Check types:
        let src: &[u8] = $src;
        let dst: &mut [$ty] = $dst;
        assert_eq!(src.len(), dst.len() * SIZE);
        for (src, dst) in src.chunks_exact(SIZE).zip(dst.iter_mut()) {
            *dst = <$ty>::$from_bytes(src.try_into().unwrap());
        }
    }};
}

/// Copies a &[$ty] $src into a &mut [u8] $dst for the endianness given by
/// $from_bytes (must be either from_be_bytes or from_le_bytes).
///
/// Panics if $src.len() * size_of::<$ty>() != $dst.len().
macro_rules! write_slice {
    ($src:expr, $dst:expr, $ty:ty, $to_bytes:ident) => {{
        const SIZE: usize = core::mem::size_of::<$ty>();
        // Check types:
        let src: &[$ty] = $src;
        let dst: &mut [u8] = $dst;
        assert_eq!(src.len() * SIZE, dst.len());
        for (src, dst) in src.iter().zip(dst.chunks_exact_mut(SIZE)) {
            dst.copy_from_slice(&src.$to_bytes());
        }
    }};
}

impl ByteOrder for BigEndian {
    #[inline]
    fn read_u16(buf: &[u8]) -> u16 {
        u16::from_be_bytes(buf[..2].try_into().unwrap())
    }

    #[inline]
    fn read_u32(buf: &[u8]) -> u32 {
        u32::from_be_bytes(buf[..4].try_into().unwrap())
    }

    #[inline]
    fn read_u64(buf: &[u8]) -> u64 {
        u64::from_be_bytes(buf[..8].try_into().unwrap())
    }

    #[inline]
    fn read_u128(buf: &[u8]) -> u128 {
        u128::from_be_bytes(buf[..16].try_into().unwrap())
    }

    #[inline]
    fn read_uint(buf: &[u8], nbytes: usize) -> u64 {
        let mut out = [0; 8];
        assert!(1 <= nbytes && nbytes <= out.len() && nbytes <= buf.len());
        let start = out.len() - nbytes;
        out[start..].copy_from_slice(&buf[..nbytes]);
        u64::from_be_bytes(out)
    }

    #[inline]
    fn read_uint128(buf: &[u8], nbytes: usize) -> u128 {
        let mut out = [0; 16];
        assert!(1 <= nbytes && nbytes <= out.len() && nbytes <= buf.len());
        let start = out.len() - nbytes;
        out[start..].copy_from_slice(&buf[..nbytes]);
        u128::from_be_bytes(out)
    }

    #[inline]
    fn write_u16(buf: &mut [u8], n: u16) {
        buf[..2].copy_from_slice(&n.to_be_bytes());
    }

    #[inline]
    fn write_u32(buf: &mut [u8], n: u32) {
        buf[..4].copy_from_slice(&n.to_be_bytes());
    }

    #[inline]
    fn write_u64(buf: &mut [u8], n: u64) {
        buf[..8].copy_from_slice(&n.to_be_bytes());
    }

    #[inline]
    fn write_u128(buf: &mut [u8], n: u128) {
        buf[..16].copy_from_slice(&n.to_be_bytes());
    }

    #[inline]
    fn write_uint(buf: &mut [u8], n: u64, nbytes: usize) {
        assert!(pack_size(n) <= nbytes && nbytes <= 8);
        assert!(nbytes <= buf.len());

        buf[..nbytes].copy_from_slice(&n.to_be_bytes()[(8 - nbytes)..]);
    }

    #[inline]
    fn write_uint128(buf: &mut [u8], n: u128, nbytes: usize) {
        assert!(pack_size128(n) <= nbytes && nbytes <= 16);
        assert!(nbytes <= buf.len());

        buf[..nbytes].copy_from_slice(&n.to_be_bytes()[(16 - nbytes)..]);
    }

    #[inline]
    fn read_u16_into(src: &[u8], dst: &mut [u16]) {
        read_slice!(src, dst, u16, from_be_bytes);
    }

    #[inline]
    fn read_u32_into(src: &[u8], dst: &mut [u32]) {
        read_slice!(src, dst, u32, from_be_bytes);
    }

    #[inline]
    fn read_u64_into(src: &[u8], dst: &mut [u64]) {
        read_slice!(src, dst, u64, from_be_bytes);
    }

    #[inline]
    fn read_u128_into(src: &[u8], dst: &mut [u128]) {
        read_slice!(src, dst, u128, from_be_bytes);
    }

    #[inline]
    fn read_i16_into(src: &[u8], dst: &mut [i16]) {
        read_slice!(src, dst, i16, from_be_bytes);
    }

    #[inline]
    fn read_i32_into(src: &[u8], dst: &mut [i32]) {
        read_slice!(src, dst, i32, from_be_bytes);
    }

    #[inline]
    fn read_i64_into(src: &[u8], dst: &mut [i64]) {
        read_slice!(src, dst, i64, from_be_bytes);
    }

    #[inline]
    fn read_i128_into(src: &[u8], dst: &mut [i128]) {
        read_slice!(src, dst, i128, from_be_bytes);
    }

    #[inline]
    fn read_f32_into(src: &[u8], dst: &mut [f32]) {
        read_slice!(src, dst, f32, from_be_bytes);
    }

    #[inline]
    fn read_f64_into(src: &[u8], dst: &mut [f64]) {
        read_slice!(src, dst, f64, from_be_bytes);
    }

    #[inline]
    fn write_u16_into(src: &[u16], dst: &mut [u8]) {
        write_slice!(src, dst, u16, to_be_bytes);
    }

    #[inline]
    fn write_u32_into(src: &[u32], dst: &mut [u8]) {
        write_slice!(src, dst, u32, to_be_bytes);
    }

    #[inline]
    fn write_u64_into(src: &[u64], dst: &mut [u8]) {
        write_slice!(src, dst, u64, to_be_bytes);
    }

    #[inline]
    fn write_u128_into(src: &[u128], dst: &mut [u8]) {
        write_slice!(src, dst, u128, to_be_bytes);
    }

    #[inline]
    fn write_i16_into(src: &[i16], dst: &mut [u8]) {
        write_slice!(src, dst, i16, to_be_bytes);
    }

    #[inline]
    fn write_i32_into(src: &[i32], dst: &mut [u8]) {
        write_slice!(src, dst, i32, to_be_bytes);
    }

    #[inline]
    fn write_i64_into(src: &[i64], dst: &mut [u8]) {
        write_slice!(src, dst, i64, to_be_bytes);
    }

    #[inline]
    fn write_i128_into(src: &[i128], dst: &mut [u8]) {
        write_slice!(src, dst, i128, to_be_bytes);
    }

    #[inline]
    fn write_f32_into(src: &[f32], dst: &mut [u8]) {
        write_slice!(src, dst, f32, to_be_bytes);
    }

    #[inline]
    fn write_f64_into(src: &[f64], dst: &mut [u8]) {
        write_slice!(src, dst, f64, to_be_bytes);
    }

    #[inline]
    fn from_slice_u16(numbers: &mut [u16]) {
        if cfg!(target_endian = "little") {
            for n in numbers {
                *n = n.to_be();
            }
        }
    }

    #[inline]
    fn from_slice_u32(numbers: &mut [u32]) {
        if cfg!(target_endian = "little") {
            for n in numbers {
                *n = n.to_be();
            }
        }
    }

    #[inline]
    fn from_slice_u64(numbers: &mut [u64]) {
        if cfg!(target_endian = "little") {
            for n in numbers {
                *n = n.to_be();
            }
        }
    }

    #[inline]
    fn from_slice_u128(numbers: &mut [u128]) {
        if cfg!(target_endian = "little") {
            for n in numbers {
                *n = n.to_be();
            }
        }
    }

    #[inline]
    fn from_slice_i16(numbers: &mut [i16]) {
        if cfg!(target_endian = "little") {
            for n in numbers {
                *n = n.to_be();
            }
        }
    }

    #[inline]
    fn from_slice_i32(numbers: &mut [i32]) {
        if cfg!(target_endian = "little") {
            for n in numbers {
                *n = n.to_be();
            }
        }
    }

    #[inline]
    fn from_slice_i64(numbers: &mut [i64]) {
        if cfg!(target_endian = "little") {
            for n in numbers {
                *n = n.to_be();
            }
        }
    }

    #[inline]
    fn from_slice_i128(numbers: &mut [i128]) {
        if cfg!(target_endian = "little") {
            for n in numbers {
                *n = n.to_be();
            }
        }
    }

    #[inline]
    fn from_slice_f32(numbers: &mut [f32]) {
        if cfg!(target_endian = "little") {
            for n in numbers {
                *n = f32::from_bits(n.to_bits().to_be());
            }
        }
    }

    #[inline]
    fn from_slice_f64(numbers: &mut [f64]) {
        if cfg!(target_endian = "little") {
            for n in numbers {
                *n = f64::from_bits(n.to_bits().to_be());
            }
        }
    }
}

impl ByteOrder for LittleEndian {
    #[inline]
    fn read_u16(buf: &[u8]) -> u16 {
        u16::from_le_bytes(buf[..2].try_into().unwrap())
    }

    #[inline]
    fn read_u32(buf: &[u8]) -> u32 {
        u32::from_le_bytes(buf[..4].try_into().unwrap())
    }

    #[inline]
    fn read_u64(buf: &[u8]) -> u64 {
        u64::from_le_bytes(buf[..8].try_into().unwrap())
    }

    #[inline]
    fn read_u128(buf: &[u8]) -> u128 {
        u128::from_le_bytes(buf[..16].try_into().unwrap())
    }

    #[inline]
    fn read_uint(buf: &[u8], nbytes: usize) -> u64 {
        let mut out = [0; 8];
        assert!(1 <= nbytes && nbytes <= out.len() && nbytes <= buf.len());
        out[..nbytes].copy_from_slice(&buf[..nbytes]);
        u64::from_le_bytes(out)
    }

    #[inline]
    fn read_uint128(buf: &[u8], nbytes: usize) -> u128 {
        let mut out = [0; 16];
        assert!(1 <= nbytes && nbytes <= out.len() && nbytes <= buf.len());
        out[..nbytes].copy_from_slice(&buf[..nbytes]);
        u128::from_le_bytes(out)
    }

    #[inline]
    fn write_u16(buf: &mut [u8], n: u16) {
        buf[..2].copy_from_slice(&n.to_le_bytes());
    }

    #[inline]
    fn write_u32(buf: &mut [u8], n: u32) {
        buf[..4].copy_from_slice(&n.to_le_bytes());
    }

    #[inline]
    fn write_u64(buf: &mut [u8], n: u64) {
        buf[..8].copy_from_slice(&n.to_le_bytes());
    }

    #[inline]
    fn write_u128(buf: &mut [u8], n: u128) {
        buf[..16].copy_from_slice(&n.to_le_bytes());
    }

    #[inline]
    fn write_uint(buf: &mut [u8], n: u64, nbytes: usize) {
        assert!(pack_size(n) <= nbytes && nbytes <= 8);
        assert!(nbytes <= buf.len());

        buf[..nbytes].copy_from_slice(&n.to_le_bytes()[..nbytes]);
    }

    #[inline]
    fn write_uint128(buf: &mut [u8], n: u128, nbytes: usize) {
        assert!(pack_size128(n) <= nbytes && nbytes <= 16);
        assert!(nbytes <= buf.len());

        buf[..nbytes].copy_from_slice(&n.to_le_bytes()[..nbytes]);
    }

    #[inline]
    fn read_u16_into(src: &[u8], dst: &mut [u16]) {
        read_slice!(src, dst, u16, from_le_bytes);
    }

    #[inline]
    fn read_u32_into(src: &[u8], dst: &mut [u32]) {
        read_slice!(src, dst, u32, from_le_bytes);
    }

    #[inline]
    fn read_u64_into(src: &[u8], dst: &mut [u64]) {
        read_slice!(src, dst, u64, from_le_bytes);
    }

    #[inline]
    fn read_u128_into(src: &[u8], dst: &mut [u128]) {
        read_slice!(src, dst, u128, from_le_bytes);
    }

    #[inline]
    fn read_i16_into(src: &[u8], dst: &mut [i16]) {
        read_slice!(src, dst, i16, from_le_bytes);
    }

    #[inline]
    fn read_i32_into(src: &[u8], dst: &mut [i32]) {
        read_slice!(src, dst, i32, from_le_bytes);
    }

    #[inline]
    fn read_i64_into(src: &[u8], dst: &mut [i64]) {
        read_slice!(src, dst, i64, from_le_bytes);
    }

    #[inline]
    fn read_i128_into(src: &[u8], dst: &mut [i128]) {
        read_slice!(src, dst, i128, from_le_bytes);
    }

    #[inline]
    fn read_f32_into(src: &[u8], dst: &mut [f32]) {
        read_slice!(src, dst, f32, from_le_bytes);
    }

    #[inline]
    fn read_f64_into(src: &[u8], dst: &mut [f64]) {
        read_slice!(src, dst, f64, from_le_bytes);
    }

    #[inline]
    fn write_u16_into(src: &[u16], dst: &mut [u8]) {
        write_slice!(src, dst, u16, to_le_bytes);
    }

    #[inline]
    fn write_u32_into(src: &[u32], dst: &mut [u8]) {
        write_slice!(src, dst, u32, to_le_bytes);
    }

    #[inline]
    fn write_u64_into(src: &[u64], dst: &mut [u8]) {
        write_slice!(src, dst, u64, to_le_bytes);
    }

    #[inline]
    fn write_u128_into(src: &[u128], dst: &mut [u8]) {
        write_slice!(src, dst, u128, to_le_bytes);
    }

    #[inline]
    fn write_i16_into(src: &[i16], dst: &mut [u8]) {
        write_slice!(src, dst, i16, to_le_bytes);
    }

    #[inline]
    fn write_i32_into(src: &[i32], dst: &mut [u8]) {
        write_slice!(src, dst, i32, to_le_bytes);
    }

    #[inline]
    fn write_i64_into(src: &[i64], dst: &mut [u8]) {
        write_slice!(src, dst, i64, to_le_bytes);
    }

    #[inline]
    fn write_i128_into(src: &[i128], dst: &mut [u8]) {
        write_slice!(src, dst, i128, to_le_bytes);
    }

    #[inline]
    fn write_f32_into(src: &[f32], dst: &mut [u8]) {
        write_slice!(src, dst, f32, to_le_bytes);
    }

    #[inline]
    fn write_f64_into(src: &[f64], dst: &mut [u8]) {
        write_slice!(src, dst, f64, to_le_bytes);
    }

    #[inline]
    fn from_slice_u16(numbers: &mut [u16]) {
        if cfg!(target_endian = "big") {
            for n in numbers {
                *n = n.to_le();
            }
        }
    }

    #[inline]
    fn from_slice_u32(numbers: &mut [u32]) {
        if cfg!(target_endian = "big") {
            for n in numbers {
                *n = n.to_le();
            }
        }
    }

    #[inline]
    fn from_slice_u64(numbers: &mut [u64]) {
        if cfg!(target_endian = "big") {
            for n in numbers {
                *n = n.to_le();
            }
        }
    }

    #[inline]
    fn from_slice_u128(numbers: &mut [u128]) {
        if cfg!(target_endian = "big") {
            for n in numbers {
                *n = n.to_le();
            }
        }
    }

    #[inline]
    fn from_slice_i16(numbers: &mut [i16]) {
        if cfg!(target_endian = "big") {
            for n in numbers {
                *n = n.to_le();
            }
        }
    }

    #[inline]
    fn from_slice_i32(numbers: &mut [i32]) {
        if cfg!(target_endian = "big") {
            for n in numbers {
                *n = n.to_le();
            }
        }
    }

    #[inline]
    fn from_slice_i64(numbers: &mut [i64]) {
        if cfg!(target_endian = "big") {
            for n in numbers {
                *n = n.to_le();
            }
        }
    }

    #[inline]
    fn from_slice_i128(numbers: &mut [i128]) {
        if cfg!(target_endian = "big") {
            for n in numbers {
                *n = n.to_le();
            }
        }
    }

    #[inline]
    fn from_slice_f32(numbers: &mut [f32]) {
        if cfg!(target_endian = "big") {
            for n in numbers {
                *n = f32::from_bits(n.to_bits().to_le());
            }
        }
    }

    #[inline]
    fn from_slice_f64(numbers: &mut [f64]) {
        if cfg!(target_endian = "big") {
            for n in numbers {
                *n = f64::from_bits(n.to_bits().to_le());
            }
        }
    }
}

#[cfg(test)]
mod test {
    use quickcheck::{Arbitrary, Gen, QuickCheck, StdGen, Testable};
    use rand::{thread_rng, Rng};

    pub const U24_MAX: u32 = 16_777_215;
    pub const I24_MAX: i32 = 8_388_607;
    pub const U48_MAX: u64 = 281_474_976_710_655;
    pub const I48_MAX: i64 = 140_737_488_355_327;

    pub const U64_MAX: u64 = ::core::u64::MAX;
    pub const I64_MAX: u64 = ::core::i64::MAX as u64;

    macro_rules! calc_max {
        ($max:expr, $bytes:expr) => {
            calc_max!($max, $bytes, 8)
        };
        ($max:expr, $bytes:expr, $maxbytes:expr) => {
            ($max - 1) >> (8 * ($maxbytes - $bytes))
        };
    }

    #[derive(Clone, Debug)]
    pub struct Wi128<T>(pub T);

    impl<T: Clone> Wi128<T> {
        pub fn clone(&self) -> T {
            self.0.clone()
        }
    }

    impl<T: PartialEq> PartialEq<T> for Wi128<T> {
        fn eq(&self, other: &T) -> bool {
            self.0.eq(other)
        }
    }

    impl Arbitrary for Wi128<u128> {
        fn arbitrary<G: Gen>(gen: &mut G) -> Wi128<u128> {
            let max = calc_max!(::core::u128::MAX, gen.size(), 16);
            let output = (gen.gen::<u64>() as u128)
                | ((gen.gen::<u64>() as u128) << 64);
            Wi128(output & (max - 1))
        }
    }

    impl Arbitrary for Wi128<i128> {
        fn arbitrary<G: Gen>(gen: &mut G) -> Wi128<i128> {
            let max = calc_max!(::core::i128::MAX, gen.size(), 16);
            let output = (gen.gen::<i64>() as i128)
                | ((gen.gen::<i64>() as i128) << 64);
            Wi128(output & (max - 1))
        }
    }

    pub fn qc_sized<A: Testable>(f: A, size: u64) {
        QuickCheck::new()
            .gen(StdGen::new(thread_rng(), size as usize))
            .tests(1_00)
            .max_tests(10_000)
            .quickcheck(f);
    }

    macro_rules! qc_byte_order {
        ($name:ident, $ty_int:ty, $max:expr,
         $bytes:expr, $read:ident, $write:ident) => {
            #[cfg(not(miri))]
            mod $name {
                #[allow(unused_imports)]
                use super::{qc_sized, Wi128};
                use crate::{
                    BigEndian, ByteOrder, LittleEndian, NativeEndian,
                };

                #[test]
                fn big_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let mut buf = [0; 16];
                        BigEndian::$write(&mut buf, n.clone(), $bytes);
                        n == BigEndian::$read(&buf[..$bytes], $bytes)
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max);
                }

                #[test]
                fn little_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let mut buf = [0; 16];
                        LittleEndian::$write(&mut buf, n.clone(), $bytes);
                        n == LittleEndian::$read(&buf[..$bytes], $bytes)
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max);
                }

                #[test]
                fn native_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let mut buf = [0; 16];
                        NativeEndian::$write(&mut buf, n.clone(), $bytes);
                        n == NativeEndian::$read(&buf[..$bytes], $bytes)
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max);
                }
            }
        };
        ($name:ident, $ty_int:ty, $max:expr,
         $read:ident, $write:ident) => {
            #[cfg(not(miri))]
            mod $name {
                #[allow(unused_imports)]
                use super::{qc_sized, Wi128};
                use crate::{
                    BigEndian, ByteOrder, LittleEndian, NativeEndian,
                };
                use core::mem::size_of;

                #[test]
                fn big_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let bytes = size_of::<$ty_int>();
                        let mut buf = [0; 16];
                        BigEndian::$write(&mut buf[16 - bytes..], n.clone());
                        n == BigEndian::$read(&buf[16 - bytes..])
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max - 1);
                }

                #[test]
                fn little_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let bytes = size_of::<$ty_int>();
                        let mut buf = [0; 16];
                        LittleEndian::$write(&mut buf[..bytes], n.clone());
                        n == LittleEndian::$read(&buf[..bytes])
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max - 1);
                }

                #[test]
                fn native_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let bytes = size_of::<$ty_int>();
                        let mut buf = [0; 16];
                        NativeEndian::$write(&mut buf[..bytes], n.clone());
                        n == NativeEndian::$read(&buf[..bytes])
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max - 1);
                }
            }
        };
    }

    qc_byte_order!(
        prop_u16,
        u16,
        ::core::u16::MAX as u64,
        read_u16,
        write_u16
    );
    qc_byte_order!(
        prop_i16,
        i16,
        ::core::i16::MAX as u64,
        read_i16,
        write_i16
    );
    qc_byte_order!(
        prop_u24,
        u32,
        crate::test::U24_MAX as u64,
        read_u24,
        write_u24
    );
    qc_byte_order!(
        prop_i24,
        i32,
        crate::test::I24_MAX as u64,
        read_i24,
        write_i24
    );
    qc_byte_order!(
        prop_u32,
        u32,
        ::core::u32::MAX as u64,
        read_u32,
        write_u32
    );
    qc_byte_order!(
        prop_i32,
        i32,
        ::core::i32::MAX as u64,
        read_i32,
        write_i32
    );
    qc_byte_order!(
        prop_u48,
        u64,
        crate::test::U48_MAX as u64,
        read_u48,
        write_u48
    );
    qc_byte_order!(
        prop_i48,
        i64,
        crate::test::I48_MAX as u64,
        read_i48,
        write_i48
    );
    qc_byte_order!(
        prop_u64,
        u64,
        ::core::u64::MAX as u64,
        read_u64,
        write_u64
    );
    qc_byte_order!(
        prop_i64,
        i64,
        ::core::i64::MAX as u64,
        read_i64,
        write_i64
    );
    qc_byte_order!(
        prop_f32,
        f32,
        ::core::u64::MAX as u64,
        read_f32,
        write_f32
    );
    qc_byte_order!(
        prop_f64,
        f64,
        ::core::i64::MAX as u64,
        read_f64,
        write_f64
    );

    qc_byte_order!(prop_u128, Wi128<u128>, 16 + 1, read_u128, write_u128);
    qc_byte_order!(prop_i128, Wi128<i128>, 16 + 1, read_i128, write_i128);

    qc_byte_order!(
        prop_uint_1,
        u64,
        calc_max!(super::U64_MAX, 1),
        1,
        read_uint,
        write_uint
    );
    qc_byte_order!(
        prop_uint_2,
        u64,
        calc_max!(super::U64_MAX, 2),
        2,
        read_uint,
        write_uint
    );
    qc_byte_order!(
        prop_uint_3,
        u64,
        calc_max!(super::U64_MAX, 3),
        3,
        read_uint,
        write_uint
    );
    qc_byte_order!(
        prop_uint_4,
        u64,
        calc_max!(super::U64_MAX, 4),
        4,
        read_uint,
        write_uint
    );
    qc_byte_order!(
        prop_uint_5,
        u64,
        calc_max!(super::U64_MAX, 5),
        5,
        read_uint,
        write_uint
    );
    qc_byte_order!(
        prop_uint_6,
        u64,
        calc_max!(super::U64_MAX, 6),
        6,
        read_uint,
        write_uint
    );
    qc_byte_order!(
        prop_uint_7,
        u64,
        calc_max!(super::U64_MAX, 7),
        7,
        read_uint,
        write_uint
    );
    qc_byte_order!(
        prop_uint_8,
        u64,
        calc_max!(super::U64_MAX, 8),
        8,
        read_uint,
        write_uint
    );

    qc_byte_order!(
        prop_uint128_1,
        Wi128<u128>,
        1,
        1,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_2,
        Wi128<u128>,
        2,
        2,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_3,
        Wi128<u128>,
        3,
        3,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_4,
        Wi128<u128>,
        4,
        4,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_5,
        Wi128<u128>,
        5,
        5,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_6,
        Wi128<u128>,
        6,
        6,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_7,
        Wi128<u128>,
        7,
        7,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_8,
        Wi128<u128>,
        8,
        8,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_9,
        Wi128<u128>,
        9,
        9,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_10,
        Wi128<u128>,
        10,
        10,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_11,
        Wi128<u128>,
        11,
        11,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_12,
        Wi128<u128>,
        12,
        12,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_13,
        Wi128<u128>,
        13,
        13,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_14,
        Wi128<u128>,
        14,
        14,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_15,
        Wi128<u128>,
        15,
        15,
        read_uint128,
        write_uint128
    );
    qc_byte_order!(
        prop_uint128_16,
        Wi128<u128>,
        16,
        16,
        read_uint128,
        write_uint128
    );

    qc_byte_order!(
        prop_int_1,
        i64,
        calc_max!(super::I64_MAX, 1),
        1,
        read_int,
        write_int
    );
    qc_byte_order!(
        prop_int_2,
        i64,
        calc_max!(super::I64_MAX, 2),
        2,
        read_int,
        write_int
    );
    qc_byte_order!(
        prop_int_3,
        i64,
        calc_max!(super::I64_MAX, 3),
        3,
        read_int,
        write_int
    );
    qc_byte_order!(
        prop_int_4,
        i64,
        calc_max!(super::I64_MAX, 4),
        4,
        read_int,
        write_int
    );
    qc_byte_order!(
        prop_int_5,
        i64,
        calc_max!(super::I64_MAX, 5),
        5,
        read_int,
        write_int
    );
    qc_byte_order!(
        prop_int_6,
        i64,
        calc_max!(super::I64_MAX, 6),
        6,
        read_int,
        write_int
    );
    qc_byte_order!(
        prop_int_7,
        i64,
        calc_max!(super::I64_MAX, 7),
        7,
        read_int,
        write_int
    );
    qc_byte_order!(
        prop_int_8,
        i64,
        calc_max!(super::I64_MAX, 8),
        8,
        read_int,
        write_int
    );

    qc_byte_order!(
        prop_int128_1,
        Wi128<i128>,
        1,
        1,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_2,
        Wi128<i128>,
        2,
        2,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_3,
        Wi128<i128>,
        3,
        3,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_4,
        Wi128<i128>,
        4,
        4,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_5,
        Wi128<i128>,
        5,
        5,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_6,
        Wi128<i128>,
        6,
        6,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_7,
        Wi128<i128>,
        7,
        7,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_8,
        Wi128<i128>,
        8,
        8,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_9,
        Wi128<i128>,
        9,
        9,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_10,
        Wi128<i128>,
        10,
        10,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_11,
        Wi128<i128>,
        11,
        11,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_12,
        Wi128<i128>,
        12,
        12,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_13,
        Wi128<i128>,
        13,
        13,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_14,
        Wi128<i128>,
        14,
        14,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_15,
        Wi128<i128>,
        15,
        15,
        read_int128,
        write_int128
    );
    qc_byte_order!(
        prop_int128_16,
        Wi128<i128>,
        16,
        16,
        read_int128,
        write_int128
    );

    // Test that all of the byte conversion functions panic when given a
    // buffer that is too small.
    //
    // These tests are critical to ensure safety, otherwise we might end up
    // with a buffer overflow.
    macro_rules! too_small {
        ($name:ident, $maximally_small:expr, $zero:expr,
         $read:ident, $write:ident) => {
            mod $name {
                use crate::{
                    BigEndian, ByteOrder, LittleEndian, NativeEndian,
                };

                #[test]
                #[should_panic]
                fn read_big_endian() {
                    let buf = [0; $maximally_small];
                    BigEndian::$read(&buf);
                }

                #[test]
                #[should_panic]
                fn read_little_endian() {
                    let buf = [0; $maximally_small];
                    LittleEndian::$read(&buf);
                }

                #[test]
                #[should_panic]
                fn read_native_endian() {
                    let buf = [0; $maximally_small];
                    NativeEndian::$read(&buf);
                }

                #[test]
                #[should_panic]
                fn write_big_endian() {
                    let mut buf = [0; $maximally_small];
                    BigEndian::$write(&mut buf, $zero);
                }

                #[test]
                #[should_panic]
                fn write_little_endian() {
                    let mut buf = [0; $maximally_small];
                    LittleEndian::$write(&mut buf, $zero);
                }

                #[test]
                #[should_panic]
                fn write_native_endian() {
                    let mut buf = [0; $maximally_small];
                    NativeEndian::$write(&mut buf, $zero);
                }
            }
        };
        ($name:ident, $maximally_small:expr, $read:ident) => {
            mod $name {
                use crate::{
                    BigEndian, ByteOrder, LittleEndian, NativeEndian,
                };

                #[test]
                #[should_panic]
                fn read_big_endian() {
                    let buf = [0; $maximally_small];
                    BigEndian::$read(&buf, $maximally_small + 1);
                }

                #[test]
                #[should_panic]
                fn read_little_endian() {
                    let buf = [0; $maximally_small];
                    LittleEndian::$read(&buf, $maximally_small + 1);
                }

                #[test]
                #[should_panic]
                fn read_native_endian() {
                    let buf = [0; $maximally_small];
                    NativeEndian::$read(&buf, $maximally_small + 1);
                }
            }
        };
    }

    too_small!(small_u16, 1, 0, read_u16, write_u16);
    too_small!(small_i16, 1, 0, read_i16, write_i16);
    too_small!(small_u32, 3, 0, read_u32, write_u32);
    too_small!(small_i32, 3, 0, read_i32, write_i32);
    too_small!(small_u64, 7, 0, read_u64, write_u64);
    too_small!(small_i64, 7, 0, read_i64, write_i64);
    too_small!(small_f32, 3, 0.0, read_f32, write_f32);
    too_small!(small_f64, 7, 0.0, read_f64, write_f64);
    too_small!(small_u128, 15, 0, read_u128, write_u128);
    too_small!(small_i128, 15, 0, read_i128, write_i128);

    too_small!(small_uint_1, 1, read_uint);
    too_small!(small_uint_2, 2, read_uint);
    too_small!(small_uint_3, 3, read_uint);
    too_small!(small_uint_4, 4, read_uint);
    too_small!(small_uint_5, 5, read_uint);
    too_small!(small_uint_6, 6, read_uint);
    too_small!(small_uint_7, 7, read_uint);

    too_small!(small_uint128_1, 1, read_uint128);
    too_small!(small_uint128_2, 2, read_uint128);
    too_small!(small_uint128_3, 3, read_uint128);
    too_small!(small_uint128_4, 4, read_uint128);
    too_small!(small_uint128_5, 5, read_uint128);
    too_small!(small_uint128_6, 6, read_uint128);
    too_small!(small_uint128_7, 7, read_uint128);
    too_small!(small_uint128_8, 8, read_uint128);
    too_small!(small_uint128_9, 9, read_uint128);
    too_small!(small_uint128_10, 10, read_uint128);
    too_small!(small_uint128_11, 11, read_uint128);
    too_small!(small_uint128_12, 12, read_uint128);
    too_small!(small_uint128_13, 13, read_uint128);
    too_small!(small_uint128_14, 14, read_uint128);
    too_small!(small_uint128_15, 15, read_uint128);

    too_small!(small_int_1, 1, read_int);
    too_small!(small_int_2, 2, read_int);
    too_small!(small_int_3, 3, read_int);
    too_small!(small_int_4, 4, read_int);
    too_small!(small_int_5, 5, read_int);
    too_small!(small_int_6, 6, read_int);
    too_small!(small_int_7, 7, read_int);

    too_small!(small_int128_1, 1, read_int128);
    too_small!(small_int128_2, 2, read_int128);
    too_small!(small_int128_3, 3, read_int128);
    too_small!(small_int128_4, 4, read_int128);
    too_small!(small_int128_5, 5, read_int128);
    too_small!(small_int128_6, 6, read_int128);
    too_small!(small_int128_7, 7, read_int128);
    too_small!(small_int128_8, 8, read_int128);
    too_small!(small_int128_9, 9, read_int128);
    too_small!(small_int128_10, 10, read_int128);
    too_small!(small_int128_11, 11, read_int128);
    too_small!(small_int128_12, 12, read_int128);
    too_small!(small_int128_13, 13, read_int128);
    too_small!(small_int128_14, 14, read_int128);
    too_small!(small_int128_15, 15, read_int128);

    // Test that reading/writing slices enforces the correct lengths.
    macro_rules! slice_lengths {
        ($name:ident, $read:ident, $write:ident,
         $num_bytes:expr, $numbers:expr) => {
            mod $name {
                use crate::{
                    BigEndian, ByteOrder, LittleEndian, NativeEndian,
                };

                #[test]
                #[should_panic]
                fn read_big_endian() {
                    let bytes = [0; $num_bytes];
                    let mut numbers = $numbers;
                    BigEndian::$read(&bytes, &mut numbers);
                }

                #[test]
                #[should_panic]
                fn read_little_endian() {
                    let bytes = [0; $num_bytes];
                    let mut numbers = $numbers;
                    LittleEndian::$read(&bytes, &mut numbers);
                }

                #[test]
                #[should_panic]
                fn read_native_endian() {
                    let bytes = [0; $num_bytes];
                    let mut numbers = $numbers;
                    NativeEndian::$read(&bytes, &mut numbers);
                }

                #[test]
                #[should_panic]
                fn write_big_endian() {
                    let mut bytes = [0; $num_bytes];
                    let numbers = $numbers;
                    BigEndian::$write(&numbers, &mut bytes);
                }

                #[test]
                #[should_panic]
                fn write_little_endian() {
                    let mut bytes = [0; $num_bytes];
                    let numbers = $numbers;
                    LittleEndian::$write(&numbers, &mut bytes);
                }

                #[test]
                #[should_panic]
                fn write_native_endian() {
                    let mut bytes = [0; $num_bytes];
                    let numbers = $numbers;
                    NativeEndian::$write(&numbers, &mut bytes);
                }
            }
        };
    }

    slice_lengths!(
        slice_len_too_small_u16,
        read_u16_into,
        write_u16_into,
        3,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_big_u16,
        read_u16_into,
        write_u16_into,
        5,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_small_i16,
        read_i16_into,
        write_i16_into,
        3,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_big_i16,
        read_i16_into,
        write_i16_into,
        5,
        [0, 0]
    );

    slice_lengths!(
        slice_len_too_small_u32,
        read_u32_into,
        write_u32_into,
        7,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_big_u32,
        read_u32_into,
        write_u32_into,
        9,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_small_i32,
        read_i32_into,
        write_i32_into,
        7,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_big_i32,
        read_i32_into,
        write_i32_into,
        9,
        [0, 0]
    );

    slice_lengths!(
        slice_len_too_small_u64,
        read_u64_into,
        write_u64_into,
        15,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_big_u64,
        read_u64_into,
        write_u64_into,
        17,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_small_i64,
        read_i64_into,
        write_i64_into,
        15,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_big_i64,
        read_i64_into,
        write_i64_into,
        17,
        [0, 0]
    );

    slice_lengths!(
        slice_len_too_small_u128,
        read_u128_into,
        write_u128_into,
        31,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_big_u128,
        read_u128_into,
        write_u128_into,
        33,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_small_i128,
        read_i128_into,
        write_i128_into,
        31,
        [0, 0]
    );
    slice_lengths!(
        slice_len_too_big_i128,
        read_i128_into,
        write_i128_into,
        33,
        [0, 0]
    );

    #[test]
    fn uint_bigger_buffer() {
        use crate::{ByteOrder, LittleEndian};
        let n = LittleEndian::read_uint(&[1, 2, 3, 4, 5, 6, 7, 8], 5);
        assert_eq!(n, 0x05_0403_0201);
    }

    #[test]
    fn regression173_array_impl() {
        use crate::{BigEndian, ByteOrder, LittleEndian};

        let xs = [0; 100];

        let x = BigEndian::read_u16(&xs);
        assert_eq!(x, 0);
        let x = BigEndian::read_u32(&xs);
        assert_eq!(x, 0);
        let x = BigEndian::read_u64(&xs);
        assert_eq!(x, 0);
        let x = BigEndian::read_u128(&xs);
        assert_eq!(x, 0);
        let x = BigEndian::read_i16(&xs);
        assert_eq!(x, 0);
        let x = BigEndian::read_i32(&xs);
        assert_eq!(x, 0);
        let x = BigEndian::read_i64(&xs);
        assert_eq!(x, 0);
        let x = BigEndian::read_i128(&xs);
        assert_eq!(x, 0);

        let x = LittleEndian::read_u16(&xs);
        assert_eq!(x, 0);
        let x = LittleEndian::read_u32(&xs);
        assert_eq!(x, 0);
        let x = LittleEndian::read_u64(&xs);
        assert_eq!(x, 0);
        let x = LittleEndian::read_u128(&xs);
        assert_eq!(x, 0);
        let x = LittleEndian::read_i16(&xs);
        assert_eq!(x, 0);
        let x = LittleEndian::read_i32(&xs);
        assert_eq!(x, 0);
        let x = LittleEndian::read_i64(&xs);
        assert_eq!(x, 0);
        let x = LittleEndian::read_i128(&xs);
        assert_eq!(x, 0);
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod stdtests {
    extern crate quickcheck;
    extern crate rand;

    use self::quickcheck::{QuickCheck, StdGen, Testable};
    use self::rand::thread_rng;

    fn qc_unsized<A: Testable>(f: A) {
        QuickCheck::new()
            .gen(StdGen::new(thread_rng(), 16))
            .tests(1_00)
            .max_tests(10_000)
            .quickcheck(f);
    }

    macro_rules! calc_max {
        ($max:expr, $bytes:expr) => {
            ($max - 1) >> (8 * (8 - $bytes))
        };
    }

    macro_rules! qc_bytes_ext {
        ($name:ident, $ty_int:ty, $max:expr,
         $bytes:expr, $read:ident, $write:ident) => {
            #[cfg(not(miri))]
            mod $name {
                #[allow(unused_imports)]
                use crate::test::{qc_sized, Wi128};
                use crate::{
                    BigEndian, LittleEndian, NativeEndian, ReadBytesExt,
                    WriteBytesExt,
                };
                use std::io::Cursor;

                #[test]
                fn big_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let mut wtr = vec![];
                        wtr.$write::<BigEndian>(n.clone()).unwrap();
                        let offset = wtr.len() - $bytes;
                        let mut rdr = Cursor::new(&mut wtr[offset..]);
                        n == rdr.$read::<BigEndian>($bytes).unwrap()
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max);
                }

                #[test]
                fn little_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let mut wtr = vec![];
                        wtr.$write::<LittleEndian>(n.clone()).unwrap();
                        let mut rdr = Cursor::new(wtr);
                        n == rdr.$read::<LittleEndian>($bytes).unwrap()
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max);
                }

                #[test]
                fn native_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let mut wtr = vec![];
                        wtr.$write::<NativeEndian>(n.clone()).unwrap();
                        let offset = if cfg!(target_endian = "big") {
                            wtr.len() - $bytes
                        } else {
                            0
                        };
                        let mut rdr = Cursor::new(&mut wtr[offset..]);
                        n == rdr.$read::<NativeEndian>($bytes).unwrap()
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max);
                }
            }
        };
        ($name:ident, $ty_int:ty, $max:expr, $read:ident, $write:ident) => {
            #[cfg(not(miri))]
            mod $name {
                #[allow(unused_imports)]
                use crate::test::{qc_sized, Wi128};
                use crate::{
                    BigEndian, LittleEndian, NativeEndian, ReadBytesExt,
                    WriteBytesExt,
                };
                use std::io::Cursor;

                #[test]
                fn big_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let mut wtr = vec![];
                        wtr.$write::<BigEndian>(n.clone()).unwrap();
                        let mut rdr = Cursor::new(wtr);
                        n == rdr.$read::<BigEndian>().unwrap()
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max - 1);
                }

                #[test]
                fn little_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let mut wtr = vec![];
                        wtr.$write::<LittleEndian>(n.clone()).unwrap();
                        let mut rdr = Cursor::new(wtr);
                        n == rdr.$read::<LittleEndian>().unwrap()
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max - 1);
                }

                #[test]
                fn native_endian() {
                    fn prop(n: $ty_int) -> bool {
                        let mut wtr = vec![];
                        wtr.$write::<NativeEndian>(n.clone()).unwrap();
                        let mut rdr = Cursor::new(wtr);
                        n == rdr.$read::<NativeEndian>().unwrap()
                    }
                    qc_sized(prop as fn($ty_int) -> bool, $max - 1);
                }
            }
        };
    }

    qc_bytes_ext!(
        prop_ext_u16,
        u16,
        ::std::u16::MAX as u64,
        read_u16,
        write_u16
    );
    qc_bytes_ext!(
        prop_ext_i16,
        i16,
        ::std::i16::MAX as u64,
        read_i16,
        write_i16
    );
    qc_bytes_ext!(
        prop_ext_u32,
        u32,
        ::std::u32::MAX as u64,
        read_u32,
        write_u32
    );
    qc_bytes_ext!(
        prop_ext_i32,
        i32,
        ::std::i32::MAX as u64,
        read_i32,
        write_i32
    );
    qc_bytes_ext!(
        prop_ext_u64,
        u64,
        ::std::u64::MAX as u64,
        read_u64,
        write_u64
    );
    qc_bytes_ext!(
        prop_ext_i64,
        i64,
        ::std::i64::MAX as u64,
        read_i64,
        write_i64
    );
    qc_bytes_ext!(
        prop_ext_f32,
        f32,
        ::std::u64::MAX as u64,
        read_f32,
        write_f32
    );
    qc_bytes_ext!(
        prop_ext_f64,
        f64,
        ::std::i64::MAX as u64,
        read_f64,
        write_f64
    );

    qc_bytes_ext!(prop_ext_u128, Wi128<u128>, 16 + 1, read_u128, write_u128);
    qc_bytes_ext!(prop_ext_i128, Wi128<i128>, 16 + 1, read_i128, write_i128);

    qc_bytes_ext!(
        prop_ext_uint_1,
        u64,
        calc_max!(crate::test::U64_MAX, 1),
        1,
        read_uint,
        write_u64
    );
    qc_bytes_ext!(
        prop_ext_uint_2,
        u64,
        calc_max!(crate::test::U64_MAX, 2),
        2,
        read_uint,
        write_u64
    );
    qc_bytes_ext!(
        prop_ext_uint_3,
        u64,
        calc_max!(crate::test::U64_MAX, 3),
        3,
        read_uint,
        write_u64
    );
    qc_bytes_ext!(
        prop_ext_uint_4,
        u64,
        calc_max!(crate::test::U64_MAX, 4),
        4,
        read_uint,
        write_u64
    );
    qc_bytes_ext!(
        prop_ext_uint_5,
        u64,
        calc_max!(crate::test::U64_MAX, 5),
        5,
        read_uint,
        write_u64
    );
    qc_bytes_ext!(
        prop_ext_uint_6,
        u64,
        calc_max!(crate::test::U64_MAX, 6),
        6,
        read_uint,
        write_u64
    );
    qc_bytes_ext!(
        prop_ext_uint_7,
        u64,
        calc_max!(crate::test::U64_MAX, 7),
        7,
        read_uint,
        write_u64
    );
    qc_bytes_ext!(
        prop_ext_uint_8,
        u64,
        calc_max!(crate::test::U64_MAX, 8),
        8,
        read_uint,
        write_u64
    );

    qc_bytes_ext!(
        prop_ext_uint128_1,
        Wi128<u128>,
        1,
        1,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_2,
        Wi128<u128>,
        2,
        2,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_3,
        Wi128<u128>,
        3,
        3,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_4,
        Wi128<u128>,
        4,
        4,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_5,
        Wi128<u128>,
        5,
        5,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_6,
        Wi128<u128>,
        6,
        6,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_7,
        Wi128<u128>,
        7,
        7,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_8,
        Wi128<u128>,
        8,
        8,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_9,
        Wi128<u128>,
        9,
        9,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_10,
        Wi128<u128>,
        10,
        10,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_11,
        Wi128<u128>,
        11,
        11,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_12,
        Wi128<u128>,
        12,
        12,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_13,
        Wi128<u128>,
        13,
        13,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_14,
        Wi128<u128>,
        14,
        14,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_15,
        Wi128<u128>,
        15,
        15,
        read_uint128,
        write_u128
    );
    qc_bytes_ext!(
        prop_ext_uint128_16,
        Wi128<u128>,
        16,
        16,
        read_uint128,
        write_u128
    );

    qc_bytes_ext!(
        prop_ext_int_1,
        i64,
        calc_max!(crate::test::I64_MAX, 1),
        1,
        read_int,
        write_i64
    );
    qc_bytes_ext!(
        prop_ext_int_2,
        i64,
        calc_max!(crate::test::I64_MAX, 2),
        2,
        read_int,
        write_i64
    );
    qc_bytes_ext!(
        prop_ext_int_3,
        i64,
        calc_max!(crate::test::I64_MAX, 3),
        3,
        read_int,
        write_i64
    );
    qc_bytes_ext!(
        prop_ext_int_4,
        i64,
        calc_max!(crate::test::I64_MAX, 4),
        4,
        read_int,
        write_i64
    );
    qc_bytes_ext!(
        prop_ext_int_5,
        i64,
        calc_max!(crate::test::I64_MAX, 5),
        5,
        read_int,
        write_i64
    );
    qc_bytes_ext!(
        prop_ext_int_6,
        i64,
        calc_max!(crate::test::I64_MAX, 6),
        6,
        read_int,
        write_i64
    );
    qc_bytes_ext!(
        prop_ext_int_7,
        i64,
        calc_max!(crate::test::I64_MAX, 1),
        7,
        read_int,
        write_i64
    );
    qc_bytes_ext!(
        prop_ext_int_8,
        i64,
        calc_max!(crate::test::I64_MAX, 8),
        8,
        read_int,
        write_i64
    );

    qc_bytes_ext!(
        prop_ext_int128_1,
        Wi128<i128>,
        1,
        1,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_2,
        Wi128<i128>,
        2,
        2,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_3,
        Wi128<i128>,
        3,
        3,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_4,
        Wi128<i128>,
        4,
        4,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_5,
        Wi128<i128>,
        5,
        5,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_6,
        Wi128<i128>,
        6,
        6,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_7,
        Wi128<i128>,
        7,
        7,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_8,
        Wi128<i128>,
        8,
        8,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_9,
        Wi128<i128>,
        9,
        9,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_10,
        Wi128<i128>,
        10,
        10,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_11,
        Wi128<i128>,
        11,
        11,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_12,
        Wi128<i128>,
        12,
        12,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_13,
        Wi128<i128>,
        13,
        13,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_14,
        Wi128<i128>,
        14,
        14,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_15,
        Wi128<i128>,
        15,
        15,
        read_int128,
        write_i128
    );
    qc_bytes_ext!(
        prop_ext_int128_16,
        Wi128<i128>,
        16,
        16,
        read_int128,
        write_i128
    );

    // Test slice serialization/deserialization.
    macro_rules! qc_slice {
        ($name:ident, $ty_int:ty, $read:ident, $write:ident, $zero:expr) => {
            #[cfg(not(miri))]
            mod $name {
                use super::qc_unsized;
                #[allow(unused_imports)]
                use crate::test::Wi128;
                use crate::{
                    BigEndian, ByteOrder, LittleEndian, NativeEndian,
                };
                use core::mem::size_of;

                #[test]
                fn big_endian() {
                    fn prop(numbers: Vec<$ty_int>) -> bool {
                        let numbers: Vec<_> =
                            numbers.into_iter().map(|x| x.clone()).collect();
                        let num_bytes = size_of::<$ty_int>() * numbers.len();
                        let mut bytes = vec![0; num_bytes];

                        BigEndian::$write(&numbers, &mut bytes);

                        let mut got = vec![$zero; numbers.len()];
                        BigEndian::$read(&bytes, &mut got);

                        numbers == got
                    }
                    qc_unsized(prop as fn(_) -> bool);
                }

                #[test]
                fn little_endian() {
                    fn prop(numbers: Vec<$ty_int>) -> bool {
                        let numbers: Vec<_> =
                            numbers.into_iter().map(|x| x.clone()).collect();
                        let num_bytes = size_of::<$ty_int>() * numbers.len();
                        let mut bytes = vec![0; num_bytes];

                        LittleEndian::$write(&numbers, &mut bytes);

                        let mut got = vec![$zero; numbers.len()];
                        LittleEndian::$read(&bytes, &mut got);

                        numbers == got
                    }
                    qc_unsized(prop as fn(_) -> bool);
                }

                #[test]
                fn native_endian() {
                    fn prop(numbers: Vec<$ty_int>) -> bool {
                        let numbers: Vec<_> =
                            numbers.into_iter().map(|x| x.clone()).collect();
                        let num_bytes = size_of::<$ty_int>() * numbers.len();
                        let mut bytes = vec![0; num_bytes];

                        NativeEndian::$write(&numbers, &mut bytes);

                        let mut got = vec![$zero; numbers.len()];
                        NativeEndian::$read(&bytes, &mut got);

                        numbers == got
                    }
                    qc_unsized(prop as fn(_) -> bool);
                }
            }
        };
    }

    qc_slice!(prop_slice_u16, u16, read_u16_into, write_u16_into, 0);
    qc_slice!(prop_slice_i16, i16, read_i16_into, write_i16_into, 0);
    qc_slice!(prop_slice_u32, u32, read_u32_into, write_u32_into, 0);
    qc_slice!(prop_slice_i32, i32, read_i32_into, write_i32_into, 0);
    qc_slice!(prop_slice_u64, u64, read_u64_into, write_u64_into, 0);
    qc_slice!(prop_slice_i64, i64, read_i64_into, write_i64_into, 0);
    qc_slice!(
        prop_slice_u128,
        Wi128<u128>,
        read_u128_into,
        write_u128_into,
        0
    );
    qc_slice!(
        prop_slice_i128,
        Wi128<i128>,
        read_i128_into,
        write_i128_into,
        0
    );

    qc_slice!(prop_slice_f32, f32, read_f32_into, write_f32_into, 0.0);
    qc_slice!(prop_slice_f64, f64, read_f64_into, write_f64_into, 0.0);
}
