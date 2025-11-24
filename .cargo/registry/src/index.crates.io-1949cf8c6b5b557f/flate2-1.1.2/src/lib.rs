//! A DEFLATE-based stream compression/decompression library
//!
//! This library provides support for compression and decompression of
//! DEFLATE-based streams:
//!
//! * the DEFLATE format itself
//! * the zlib format
//! * gzip
//!
//! These three formats are all closely related and largely only differ in their
//! headers/footers. This crate has three types in each submodule for dealing
//! with these three formats.
//!
//! # Implementation
//!
//! In addition to supporting three formats, this crate supports several different
//! backends, controlled through this crate's features:
//!
//! * `default`, or `rust_backend` - this implementation uses the `miniz_oxide`
//!   crate which is a port of `miniz.c` to Rust. This feature does not
//!   require a C compiler, and only uses safe Rust code.
//!
//! * `zlib-rs` - this implementation utilizes the `zlib-rs` crate, a Rust rewrite of zlib.
//!   This backend is the fastest, at the cost of some `unsafe` Rust code.
//!
//! Several backends implemented in C are also available.
//! These are useful in case you are already using a specific C implementation
//! and need the result of compression to be bit-identical.
//! See the crate's README for details on the available C backends.
//!
//! The `zlib-rs` backend typically outperforms all the C implementations.
//!
//! # Organization
//!
//! This crate consists mainly of three modules, [`read`], [`write`], and
//! [`bufread`]. Each module contains a number of types used to encode and
//! decode various streams of data.
//!
//! All types in the [`write`] module work on instances of [`Write`][write],
//! whereas all types in the [`read`] module work on instances of
//! [`Read`][read] and [`bufread`] works with [`BufRead`][bufread]. If you
//! are decoding directly from a `&[u8]`, use the [`bufread`] types.
//!
//! ```
//! use flate2::write::GzEncoder;
//! use flate2::Compression;
//! use std::io;
//! use std::io::prelude::*;
//!
//! # fn main() { let _ = run(); }
//! # fn run() -> io::Result<()> {
//! let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
//! encoder.write_all(b"Example")?;
//! # Ok(())
//! # }
//! ```
//!
//!
//! Other various types are provided at the top-level of the crate for
//! management and dealing with encoders/decoders. Also note that types which
//! operate over a specific trait often implement the mirroring trait as well.
//! For example a `flate2::read::DeflateDecoder<T>` *also* implements the
//! `Write` trait if `T: Write`. That is, the "dual trait" is forwarded directly
//! to the underlying object if available.
//!
//! # About multi-member Gzip files
//!
//! While most `gzip` files one encounters will have a single *member* that can be read
//! with the [`GzDecoder`], there may be some files which have multiple members.
//!
//! A [`GzDecoder`] will only read the first member of gzip data, which may unexpectedly
//! provide partial results when a multi-member gzip file is encountered. `GzDecoder` is appropriate
//! for data that is designed to be read as single members from a multi-member file. `bufread::GzDecoder`
//! and `write::GzDecoder` also allow non-gzip data following gzip data to be handled.
//!
//! The [`MultiGzDecoder`] on the other hand will decode all members of a `gzip` file
//! into one consecutive stream of bytes, which hides the underlying *members* entirely.
//! If a file contains non-gzip data after the gzip data, MultiGzDecoder will
//! emit an error after decoding the gzip data. This behavior matches the `gzip`,
//! `gunzip`, and `zcat` command line tools.
//!
//! [`read`]: read/index.html
//! [`bufread`]: bufread/index.html
//! [`write`]: write/index.html
//! [read]: https://doc.rust-lang.org/std/io/trait.Read.html
//! [write]: https://doc.rust-lang.org/std/io/trait.Write.html
//! [bufread]: https://doc.rust-lang.org/std/io/trait.BufRead.html
//! [`GzDecoder`]: read/struct.GzDecoder.html
//! [`MultiGzDecoder`]: read/struct.MultiGzDecoder.html
#![doc(html_root_url = "https://docs.rs/flate2/0.2")]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![allow(trivial_numeric_casts)]
#![cfg_attr(test, deny(warnings))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(not(feature = "any_impl",))]
compile_error!("You need to choose a zlib backend");

pub use crate::crc::{Crc, CrcReader, CrcWriter};
pub use crate::gz::GzBuilder;
pub use crate::gz::GzHeader;
pub use crate::mem::{Compress, CompressError, Decompress, DecompressError, Status};
pub use crate::mem::{FlushCompress, FlushDecompress};

mod bufreader;
mod crc;
mod deflate;
mod ffi;
mod gz;
mod mem;
mod zio;
mod zlib;

/// Types which operate over [`Read`] streams, both encoders and decoders for
/// various formats.
///
/// Note that the `read` decoder types may read past the end of the compressed
/// data while decoding. If the caller requires subsequent reads to start
/// immediately following the compressed data  wrap the `Read` type in a
/// [`BufReader`] and use the `BufReader` with the equivalent decoder from the
/// `bufread` module and also for the subsequent reads.
///
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
/// [`BufReader`]: https://doc.rust-lang.org/std/io/struct.BufReader.html
pub mod read {
    pub use crate::deflate::read::DeflateDecoder;
    pub use crate::deflate::read::DeflateEncoder;
    pub use crate::gz::read::GzDecoder;
    pub use crate::gz::read::GzEncoder;
    pub use crate::gz::read::MultiGzDecoder;
    pub use crate::zlib::read::ZlibDecoder;
    pub use crate::zlib::read::ZlibEncoder;
}

/// Types which operate over [`Write`] streams, both encoders and decoders for
/// various formats.
///
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
pub mod write {
    pub use crate::deflate::write::DeflateDecoder;
    pub use crate::deflate::write::DeflateEncoder;
    pub use crate::gz::write::GzDecoder;
    pub use crate::gz::write::GzEncoder;
    pub use crate::gz::write::MultiGzDecoder;
    pub use crate::zlib::write::ZlibDecoder;
    pub use crate::zlib::write::ZlibEncoder;
}

/// Types which operate over [`BufRead`] streams, both encoders and decoders for
/// various formats.
///
/// [`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
pub mod bufread {
    pub use crate::deflate::bufread::DeflateDecoder;
    pub use crate::deflate::bufread::DeflateEncoder;
    pub use crate::gz::bufread::GzDecoder;
    pub use crate::gz::bufread::GzEncoder;
    pub use crate::gz::bufread::MultiGzDecoder;
    pub use crate::zlib::bufread::ZlibDecoder;
    pub use crate::zlib::bufread::ZlibEncoder;
}

fn _assert_send_sync() {
    fn _assert_send_sync<T: Send + Sync>() {}

    _assert_send_sync::<read::DeflateEncoder<&[u8]>>();
    _assert_send_sync::<read::DeflateDecoder<&[u8]>>();
    _assert_send_sync::<read::ZlibEncoder<&[u8]>>();
    _assert_send_sync::<read::ZlibDecoder<&[u8]>>();
    _assert_send_sync::<read::GzEncoder<&[u8]>>();
    _assert_send_sync::<read::GzDecoder<&[u8]>>();
    _assert_send_sync::<read::MultiGzDecoder<&[u8]>>();
    _assert_send_sync::<write::DeflateEncoder<Vec<u8>>>();
    _assert_send_sync::<write::DeflateDecoder<Vec<u8>>>();
    _assert_send_sync::<write::ZlibEncoder<Vec<u8>>>();
    _assert_send_sync::<write::ZlibDecoder<Vec<u8>>>();
    _assert_send_sync::<write::GzEncoder<Vec<u8>>>();
    _assert_send_sync::<write::GzDecoder<Vec<u8>>>();
}

/// When compressing data, the compression level can be specified by a value in
/// this struct.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Compression(u32);

impl Compression {
    /// Creates a new description of the compression level with an explicitly
    /// specified integer.
    ///
    /// The integer here is typically on a scale of 0-9 where 0 means "no
    /// compression" and 9 means "take as long as you'd like".
    ///
    /// ### Backend differences
    ///
    /// The [`miniz_oxide`](https://crates.io/crates/miniz_oxide) backend for flate2
    /// does not support level 0 or `Compression::none()`. Instead it interprets them
    /// as the default compression level, which is quite slow.
    /// `Compression::fast()` should be used instead.
    ///
    /// `miniz_oxide` also supports a non-compliant compression level 10.
    /// It is even slower and may result in higher compression, but
    /// **only miniz_oxide will be able to read the data** compressed with level 10.
    /// Do **not** use level 10 if you need other software to be able to read it!
    pub const fn new(level: u32) -> Compression {
        Compression(level)
    }

    /// No compression is to be performed, this may actually inflate data
    /// slightly when encoding.
    pub const fn none() -> Compression {
        Compression(0)
    }

    /// Optimize for the best speed of encoding.
    pub const fn fast() -> Compression {
        Compression(1)
    }

    /// Optimize for the size of data being encoded.
    pub const fn best() -> Compression {
        Compression(9)
    }

    /// Returns an integer representing the compression level, typically on a
    /// scale of 0-9. See [`new`](Self::new) for details about compression levels.
    pub fn level(&self) -> u32 {
        self.0
    }
}

impl Default for Compression {
    fn default() -> Compression {
        Compression(6)
    }
}

#[cfg(test)]
fn random_bytes() -> impl Iterator<Item = u8> {
    use rand::Rng;
    use std::iter;

    iter::repeat(()).map(|_| rand::rng().random())
}
