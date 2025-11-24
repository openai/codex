//! A pure rust replacement for the [miniz](https://github.com/richgel999/miniz)
//! DEFLATE/zlib encoder/decoder.
//! Used a rust back-end for the
//! [flate2](https://github.com/alexcrichton/flate2-rs) crate.
//!
#![cfg_attr(
    feature = "with-alloc",
    doc = r##"
# Usage
## Simple compression/decompression:
``` rust

use miniz_oxide::inflate::decompress_to_vec;
use miniz_oxide::deflate::compress_to_vec;

fn roundtrip(data: &[u8]) {
    let compressed = compress_to_vec(data, 6);
    let decompressed = decompress_to_vec(compressed.as_slice()).expect("Failed to decompress!");
#   let _ = decompressed;
}

# roundtrip(b"Test_data test data lalalal blabla");
"##
)]
#![forbid(unsafe_code)]
#![cfg_attr(all(not(feature = "std"), not(feature = "serde")), no_std)]

#[cfg(feature = "with-alloc")]
extern crate alloc;

#[cfg(feature = "with-alloc")]
pub mod deflate;
pub mod inflate;
#[cfg(feature = "serde")]
pub mod serde;
mod shared;

pub use crate::shared::update_adler32 as mz_adler32_oxide;
pub use crate::shared::{MZ_ADLER32_INIT, MZ_DEFAULT_WINDOW_BITS};

/// A list of flush types.
///
/// See <http://www.bolet.org/~pornin/deflate-flush.html> for more in-depth info.
#[repr(i32)]
#[derive(Copy, Clone, PartialEq, Eq)]
#[cfg_attr(not(feature = "rustc-dep-of-std"), derive(Hash, Debug))]
pub enum MZFlush {
    /// Don't force any flushing.
    /// Used when more input data is expected.
    None = 0,
    /// Zlib partial flush.
    /// Currently treated as [`Sync`].
    Partial = 1,
    /// Finish compressing the currently buffered data, and output an empty raw block.
    /// Has no use in decompression.
    Sync = 2,
    /// Same as [`Sync`], but resets the compression dictionary so that further compressed
    /// data does not depend on data compressed before the flush.
    ///
    /// Has no use in decompression, and is an error to supply in that case.
    Full = 3,
    /// Attempt to flush the remaining data and end the stream.
    Finish = 4,
    /// Not implemented.
    Block = 5,
}

impl MZFlush {
    /// Create an MZFlush value from an integer value.
    ///
    /// Returns `MZError::Param` on invalid values.
    pub fn new(flush: i32) -> Result<Self, MZError> {
        match flush {
            0 => Ok(MZFlush::None),
            1 | 2 => Ok(MZFlush::Sync),
            3 => Ok(MZFlush::Full),
            4 => Ok(MZFlush::Finish),
            _ => Err(MZError::Param),
        }
    }
}

/// A list of miniz successful status codes.
///
/// These are emitted as the [`Ok`] side of a [`MZResult`] in the [`StreamResult`] returned from
/// [`deflate::stream::deflate()`] or [`inflate::stream::inflate()`].
#[repr(i32)]
#[derive(Copy, Clone, PartialEq, Eq)]
#[cfg_attr(not(feature = "rustc-dep-of-std"), derive(Hash, Debug))]
pub enum MZStatus {
    /// Operation succeeded.
    ///
    /// Some data was decompressed or compressed; see the byte counters in the [`StreamResult`] for
    /// details.
    Ok = 0,

    /// Operation succeeded and end of deflate stream was found.
    ///
    /// X-ref [`TINFLStatus::Done`][inflate::TINFLStatus::Done] or
    /// [`TDEFLStatus::Done`][deflate::core::TDEFLStatus::Done] for `inflate` or `deflate`
    /// respectively.
    StreamEnd = 1,

    /// Unused
    NeedDict = 2,
}

/// A list of miniz failed status codes.
///
/// These are emitted as the [`Err`] side of a [`MZResult`] in the [`StreamResult`] returned from
/// [`deflate::stream::deflate()`] or [`inflate::stream::inflate()`].
#[repr(i32)]
#[cfg_attr(not(feature = "rustc-dep-of-std"), derive(Hash, Debug))]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum MZError {
    /// Unused
    ErrNo = -1,

    /// General stream error.
    ///
    /// See [`inflate::stream::inflate()`] docs for details of how it can occur there.
    ///
    /// See [`deflate::stream::deflate()`] docs for how it can in principle occur there, though it's
    /// believed impossible in practice.
    Stream = -2,

    /// Error in inflation; see [`inflate::stream::inflate()`] for details.
    ///
    /// Not returned from [`deflate::stream::deflate()`].
    Data = -3,

    /// Unused
    Mem = -4,

    /// Buffer-related error.
    ///
    /// See the docs of [`deflate::stream::deflate()`] or [`inflate::stream::inflate()`] for details
    /// of when it would trigger in the one you're using.
    Buf = -5,

    /// Unused
    Version = -6,

    /// Bad parameters.
    ///
    /// This can be returned from [`deflate::stream::deflate()`] in the case of bad parameters.  See
    /// [`TDEFLStatus::BadParam`][deflate::core::TDEFLStatus::BadParam].
    Param = -10_000,
}

/// How compressed data is wrapped.
#[derive(Copy, Clone, PartialEq, Eq)]
#[cfg_attr(not(feature = "rustc-dep-of-std"), derive(Hash, Debug))]
#[non_exhaustive]
pub enum DataFormat {
    /// Wrapped using the [zlib](http://www.zlib.org/rfc-zlib.html) format.
    Zlib,
    /// Zlib wrapped but ignore and don't compute the adler32 checksum.
    /// Currently only used for inflate, behaves the same as Zlib for compression.
    ZLibIgnoreChecksum,
    /// Raw DEFLATE.
    Raw,
}

#[cfg(not(feature = "rustc-dep-of-std"))]
impl DataFormat {
    pub fn from_window_bits(window_bits: i32) -> DataFormat {
        if window_bits > 0 {
            DataFormat::Zlib
        } else {
            DataFormat::Raw
        }
    }

    pub fn to_window_bits(self) -> i32 {
        match self {
            DataFormat::Zlib | DataFormat::ZLibIgnoreChecksum => shared::MZ_DEFAULT_WINDOW_BITS,
            DataFormat::Raw => -shared::MZ_DEFAULT_WINDOW_BITS,
        }
    }
}

/// `Result` alias for all miniz status codes both successful and failed.
pub type MZResult = Result<MZStatus, MZError>;

/// A structure containing the result of a call to the inflate or deflate streaming functions.
#[cfg(not(feature = "rustc-dep-of-std"))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct StreamResult {
    /// The number of bytes consumed from the input slice.
    pub bytes_consumed: usize,
    /// The number of bytes written to the output slice.
    pub bytes_written: usize,
    /// The return status of the call.
    pub status: MZResult,
}

#[cfg(not(feature = "rustc-dep-of-std"))]
impl StreamResult {
    #[inline]
    pub const fn error(error: MZError) -> StreamResult {
        StreamResult {
            bytes_consumed: 0,
            bytes_written: 0,
            status: Err(error),
        }
    }
}

#[cfg(not(feature = "rustc-dep-of-std"))]
impl core::convert::From<StreamResult> for MZResult {
    fn from(res: StreamResult) -> Self {
        res.status
    }
}

#[cfg(not(feature = "rustc-dep-of-std"))]
impl core::convert::From<&StreamResult> for MZResult {
    fn from(res: &StreamResult) -> Self {
        res.status
    }
}
