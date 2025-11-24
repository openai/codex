//! # simd-adler32
//!
//! A SIMD-accelerated Adler-32 hash algorithm implementation.
//!
//! ## Features
//!
//! - No dependencies
//! - Support `no_std` (with `default-features = false`)
//! - Runtime CPU feature detection (when `std` enabled)
//! - Blazing fast performance on as many targets as possible (currently only x86 and x86_64)
//! - Default to scalar implementation when simd not available
//!
//! ## Quick start
//!
//! > Cargo.toml
//!
//! ```toml
//! [dependencies]
//! simd-adler32 = "*"
//! ```
//!
//! > example.rs
//!
//! ```rust
//! use simd_adler32::Adler32;
//!
//! let mut adler = Adler32::new();
//! adler.write(b"rust is pretty cool, man");
//! let hash = adler.finish();
//!
//! println!("{}", hash);
//! // 1921255656
//! ```
//!
//! ## Feature flags
//!
//! * `std` - Enabled by default
//!
//! Enables std support, see [CPU Feature Detection](#cpu-feature-detection) for runtime
//! detection support.
//! * `nightly`
//!
//! Enables nightly features required for avx512 support.
//!
//! * `const-generics` - Enabled by default
//!
//! Enables const-generics support allowing for user-defined array hashing by value.  See
//! [`Adler32Hash`] for details.
//!
//! ## Support
//!
//! **CPU Features**
//!
//! | impl | arch             | feature |
//! | ---- | ---------------- | ------- |
//! | ✅   | `x86`, `x86_64`  | avx512  |
//! | ✅   | `x86`, `x86_64`  | avx2    |
//! | ✅   | `x86`, `x86_64`  | ssse3   |
//! | ✅   | `x86`, `x86_64`  | sse2    |
//! | 🚧   | `arm`, `aarch64` | neon    |
//! |      | `wasm32`         | simd128 |
//!
//! **MSRV** `1.36.0`\*\*
//!
//! Minimum supported rust version is tested before a new version is published. [**] Feature
//! `const-generics` needs to disabled to build on rustc versions `<1.51` which can be done
//! by updating your dependency definition to the following.
//!
//! ## CPU Feature Detection
//! simd-adler32 supports both runtime and compile time CPU feature detection using the
//! `std::is_x86_feature_detected` macro when the `Adler32` struct is instantiated with
//! the `new` fn.  
//!
//! Without `std` feature enabled simd-adler32 falls back to compile time feature detection
//! using `target-feature` or `target-cpu` flags supplied to rustc. See [https://rust-lang.github.io/packed_simd/perf-guide/target-feature/rustflags.html](https://rust-lang.github.io/packed_simd/perf-guide/target-feature/rustflags.html)
//! for more information.
//!
//! Feature detection tries to use the fastest supported feature first.
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "nightly", feature(stdsimd, avx512_target_feature))]

#[doc(hidden)]
pub mod hash;
#[doc(hidden)]
pub mod imp;

pub use hash::*;
use imp::{get_imp, Adler32Imp};

/// An adler32 hash generator type.
#[derive(Clone)]
pub struct Adler32 {
  a: u16,
  b: u16,
  update: Adler32Imp,
}

impl Adler32 {
  /// Constructs a new `Adler32`.
  ///
  /// Potential overhead here due to runtime feature detection although in testing on 100k
  /// and 10k random byte arrays it was not really noticeable.
  ///
  /// # Examples
  /// ```rust
  /// use simd_adler32::Adler32;
  ///
  /// let mut adler = Adler32::new();
  /// ```
  pub fn new() -> Self {
    Default::default()
  }

  /// Constructs a new `Adler32` using existing checksum.
  ///
  /// Potential overhead here due to runtime feature detection although in testing on 100k
  /// and 10k random byte arrays it was not really noticeable.
  ///
  /// # Examples
  /// ```rust
  /// use simd_adler32::Adler32;
  ///
  /// let mut adler = Adler32::from_checksum(0xdeadbeaf);
  /// ```
  pub fn from_checksum(checksum: u32) -> Self {
    Self {
      a: checksum as u16,
      b: (checksum >> 16) as u16,
      update: get_imp(),
    }
  }

  /// Computes hash for supplied data and stores results in internal state.
  pub fn write(&mut self, data: &[u8]) {
    let (a, b) = (self.update)(self.a, self.b, data);

    self.a = a;
    self.b = b;
  }

  /// Returns the hash value for the values written so far.
  ///
  /// Despite its name, the method does not reset the hasher’s internal state. Additional
  /// writes will continue from the current value. If you need to start a fresh hash
  /// value, you will have to use `reset`.
  pub fn finish(&self) -> u32 {
    (u32::from(self.b) << 16) | u32::from(self.a)
  }

  /// Resets the internal state.
  pub fn reset(&mut self) {
    self.a = 1;
    self.b = 0;
  }
}

/// Compute Adler-32 hash on `Adler32Hash` type.
///
/// # Arguments
/// * `hash` - A Adler-32 hash-able type.
///
/// # Examples
/// ```rust
/// use simd_adler32::adler32;
///
/// let hash = adler32(b"Adler-32");
/// println!("{}", hash); // 800813569
/// ```
pub fn adler32<H: Adler32Hash>(hash: &H) -> u32 {
  hash.hash()
}

/// A Adler-32 hash-able type.
pub trait Adler32Hash {
  /// Feeds this value into `Adler32`.
  fn hash(&self) -> u32;
}

impl Default for Adler32 {
  fn default() -> Self {
    Self {
      a: 1,
      b: 0,
      update: get_imp(),
    }
  }
}

#[cfg(feature = "std")]
pub mod read {
  //! Reader-based hashing.
  //!
  //! # Example
  //! ```rust
  //! use std::io::Cursor;
  //! use simd_adler32::read::adler32;
  //!
  //! let mut reader = Cursor::new(b"Hello there");
  //! let hash = adler32(&mut reader).unwrap();
  //!
  //! println!("{}", hash) // 800813569
  //! ```
  use crate::Adler32;
  use std::io::{Read, Result};

  /// Compute Adler-32 hash on reader until EOF.
  ///
  /// # Example
  /// ```rust
  /// use std::io::Cursor;
  /// use simd_adler32::read::adler32;
  ///
  /// let mut reader = Cursor::new(b"Hello there");
  /// let hash = adler32(&mut reader).unwrap();
  ///
  /// println!("{}", hash) // 800813569
  /// ```
  pub fn adler32<R: Read>(reader: &mut R) -> Result<u32> {
    let mut hash = Adler32::new();
    let mut buf = [0; 4096];

    loop {
      match reader.read(&mut buf) {
        Ok(0) => return Ok(hash.finish()),
        Ok(n) => {
          hash.write(&buf[..n]);
        }
        Err(err) => return Err(err),
      }
    }
  }
}

#[cfg(feature = "std")]
pub mod bufread {
  //! BufRead-based hashing.
  //!
  //! Separate `BufRead` trait implemented to allow for custom buffer size optimization.
  //!
  //! # Example
  //! ```rust
  //! use std::io::{Cursor, BufReader};
  //! use simd_adler32::bufread::adler32;
  //!
  //! let mut reader = Cursor::new(b"Hello there");
  //! let mut reader = BufReader::new(reader);
  //! let hash = adler32(&mut reader).unwrap();
  //!
  //! println!("{}", hash) // 800813569
  //! ```
  use crate::Adler32;
  use std::io::{BufRead, ErrorKind, Result};

  /// Compute Adler-32 hash on buf reader until EOF.
  ///
  /// # Example
  /// ```rust
  /// use std::io::{Cursor, BufReader};
  /// use simd_adler32::bufread::adler32;
  ///
  /// let mut reader = Cursor::new(b"Hello there");
  /// let mut reader = BufReader::new(reader);
  /// let hash = adler32(&mut reader).unwrap();
  ///
  /// println!("{}", hash) // 800813569
  /// ```
  pub fn adler32<R: BufRead>(reader: &mut R) -> Result<u32> {
    let mut hash = Adler32::new();

    loop {
      let consumed = match reader.fill_buf() {
        Ok(buf) => {
          if buf.is_empty() {
            return Ok(hash.finish());
          }

          hash.write(buf);
          buf.len()
        }
        Err(err) => match err.kind() {
          ErrorKind::Interrupted => continue,
          ErrorKind::UnexpectedEof => return Ok(hash.finish()),
          _ => return Err(err),
        },
      };

      reader.consume(consumed);
    }
  }
}

#[cfg(test)]
mod tests {
  #[test]
  fn test_from_checksum() {
    let buf = b"rust is pretty cool man";
    let sum = 0xdeadbeaf;

    let mut simd = super::Adler32::from_checksum(sum);
    let mut adler = adler::Adler32::from_checksum(sum);

    simd.write(buf);
    adler.write_slice(buf);

    let simd = simd.finish();
    let scalar = adler.checksum();

    assert_eq!(simd, scalar);
  }
}
