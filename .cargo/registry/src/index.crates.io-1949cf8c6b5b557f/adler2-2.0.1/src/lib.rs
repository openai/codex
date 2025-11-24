//! Adler-32 checksum implementation.
//!
//! This implementation features:
//!
//! - Permissively licensed (0BSD) clean-room implementation.
//! - Zero dependencies.
//! - Zero `unsafe`.
//! - Decent performance (3-4 GB/s).
//! - `#![no_std]` support (with `default-features = false`).

#![doc(html_root_url = "https://docs.rs/adler2/2.0.0")]
// Deny a few warnings in doctests, since rustdoc `allow`s many warnings by default
#![doc(test(attr(deny(unused_imports, unused_must_use))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_debug_implementations)]
#![forbid(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate core as std;

mod algo;

use std::hash::Hasher;

#[cfg(feature = "std")]
use std::io::{self, BufRead};

/// Adler-32 checksum calculator.
///
/// An instance of this type is equivalent to an Adler-32 checksum: It can be created in the default
/// state via [`new`] (or the provided `Default` impl), or from a precalculated checksum via
/// [`from_checksum`], and the currently stored checksum can be fetched via [`checksum`].
///
/// This type also implements `Hasher`, which makes it easy to calculate Adler-32 checksums of any
/// type that implements or derives `Hash`. This also allows using Adler-32 in a `HashMap`, although
/// that is not recommended (while every checksum is a hash function, they are not necessarily a
/// good one).
///
/// # Examples
///
/// Basic, piecewise checksum calculation:
///
/// ```
/// use adler2::Adler32;
///
/// let mut adler = Adler32::new();
///
/// adler.write_slice(&[0, 1, 2]);
/// adler.write_slice(&[3, 4, 5]);
///
/// assert_eq!(adler.checksum(), 0x00290010);
/// ```
///
/// Using `Hash` to process structures:
///
/// ```
/// use std::hash::Hash;
/// use adler2::Adler32;
///
/// #[derive(Hash)]
/// struct Data {
///     byte: u8,
///     word: u16,
///     big: u64,
/// }
///
/// let mut adler = Adler32::new();
///
/// let data = Data { byte: 0x1F, word: 0xABCD, big: !0 };
/// data.hash(&mut adler);
///
/// // hash value depends on architecture endianness
/// if cfg!(target_endian = "little") {
///     assert_eq!(adler.checksum(), 0x33410990);
/// }
/// if cfg!(target_endian = "big") {
///     assert_eq!(adler.checksum(), 0x331F0990);
/// }
///
/// ```
///
/// [`new`]: #method.new
/// [`from_checksum`]: #method.from_checksum
/// [`checksum`]: #method.checksum
#[derive(Debug, Copy, Clone)]
pub struct Adler32 {
    a: u16,
    b: u16,
}

impl Adler32 {
    /// Creates a new Adler-32 instance with default state.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an `Adler32` instance from a precomputed Adler-32 checksum.
    ///
    /// This allows resuming checksum calculation without having to keep the `Adler32` instance
    /// around.
    ///
    /// # Example
    ///
    /// ```
    /// # use adler2::Adler32;
    /// let parts = [
    ///     "rust",
    ///     "acean",
    /// ];
    /// let whole = adler2::adler32_slice(b"rustacean");
    ///
    /// let mut sum = Adler32::new();
    /// sum.write_slice(parts[0].as_bytes());
    /// let partial = sum.checksum();
    ///
    /// // ...later
    ///
    /// let mut sum = Adler32::from_checksum(partial);
    /// sum.write_slice(parts[1].as_bytes());
    /// assert_eq!(sum.checksum(), whole);
    /// ```
    #[inline]
    pub const fn from_checksum(sum: u32) -> Self {
        Adler32 {
            a: sum as u16,
            b: (sum >> 16) as u16,
        }
    }

    /// Returns the calculated checksum at this point in time.
    #[inline]
    pub fn checksum(&self) -> u32 {
        (u32::from(self.b) << 16) | u32::from(self.a)
    }

    /// Adds `bytes` to the checksum calculation.
    ///
    /// If efficiency matters, this should be called with Byte slices that contain at least a few
    /// thousand Bytes.
    pub fn write_slice(&mut self, bytes: &[u8]) {
        self.compute(bytes);
    }
}

impl Default for Adler32 {
    #[inline]
    fn default() -> Self {
        Adler32 { a: 1, b: 0 }
    }
}

impl Hasher for Adler32 {
    #[inline]
    fn finish(&self) -> u64 {
        u64::from(self.checksum())
    }

    fn write(&mut self, bytes: &[u8]) {
        self.write_slice(bytes);
    }
}

/// Calculates the Adler-32 checksum of a byte slice.
///
/// This is a convenience function around the [`Adler32`] type.
///
/// [`Adler32`]: struct.Adler32.html
pub fn adler32_slice(data: &[u8]) -> u32 {
    let mut h = Adler32::new();
    h.write_slice(data);
    h.checksum()
}

/// Calculates the Adler-32 checksum of a `BufRead`'s contents.
///
/// The passed `BufRead` implementor will be read until it reaches EOF (or until it reports an
/// error).
///
/// If you only have a `Read` implementor, you can wrap it in `std::io::BufReader` before calling
/// this function.
///
/// # Errors
///
/// Any error returned by the reader are bubbled up by this function.
///
/// # Examples
///
/// ```no_run
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// use adler2::adler32;
///
/// use std::fs::File;
/// use std::io::BufReader;
///
/// let file = File::open("input.txt")?;
/// let mut file = BufReader::new(file);
///
/// adler32(&mut file)?;
/// # Ok(()) }
/// # fn main() { run().unwrap() }
/// ```
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub fn adler32<R: BufRead>(mut reader: R) -> io::Result<u32> {
    let mut h = Adler32::new();
    loop {
        let len = {
            let buf = reader.fill_buf()?;
            if buf.is_empty() {
                return Ok(h.checksum());
            }

            h.write_slice(buf);
            buf.len()
        };
        reader.consume(len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zeroes() {
        assert_eq!(adler32_slice(&[]), 1);
        assert_eq!(adler32_slice(&[0]), 1 | 1 << 16);
        assert_eq!(adler32_slice(&[0, 0]), 1 | 2 << 16);
        assert_eq!(adler32_slice(&[0; 100]), 0x00640001);
        assert_eq!(adler32_slice(&[0; 1024]), 0x04000001);
        assert_eq!(adler32_slice(&[0; 1024 * 1024]), 0x00f00001);
    }

    #[test]
    fn ones() {
        assert_eq!(adler32_slice(&[0xff; 1024]), 0x79a6fc2e);
        assert_eq!(adler32_slice(&[0xff; 1024 * 1024]), 0x8e88ef11);
    }

    #[test]
    fn mixed() {
        assert_eq!(adler32_slice(&[1]), 2 | 2 << 16);
        assert_eq!(adler32_slice(&[40]), 41 | 41 << 16);

        assert_eq!(adler32_slice(&[0xA5; 1024 * 1024]), 0xd5009ab1);
    }

    /// Example calculation from https://en.wikipedia.org/wiki/Adler-32.
    #[test]
    fn wiki() {
        assert_eq!(adler32_slice(b"Wikipedia"), 0x11E60398);
    }

    #[test]
    fn resume() {
        let mut adler = Adler32::new();
        adler.write_slice(&[0xff; 1024]);
        let partial = adler.checksum();
        assert_eq!(partial, 0x79a6fc2e); // from above
        adler.write_slice(&[0xff; 1024 * 1024 - 1024]);
        assert_eq!(adler.checksum(), 0x8e88ef11); // from above

        // Make sure that we can resume computing from the partial checksum via `from_checksum`.
        let mut adler = Adler32::from_checksum(partial);
        adler.write_slice(&[0xff; 1024 * 1024 - 1024]);
        assert_eq!(adler.checksum(), 0x8e88ef11); // from above
    }

    #[cfg(feature = "std")]
    #[test]
    fn bufread() {
        use std::io::BufReader;
        fn test(data: &[u8], checksum: u32) {
            // `BufReader` uses an 8 KB buffer, so this will test buffer refilling.
            let mut buf = BufReader::new(data);
            let real_sum = adler32(&mut buf).unwrap();
            assert_eq!(checksum, real_sum);
        }

        test(&[], 1);
        test(&[0; 1024], 0x04000001);
        test(&[0; 1024 * 1024], 0x00f00001);
        test(&[0xA5; 1024 * 1024], 0xd5009ab1);
    }
}
