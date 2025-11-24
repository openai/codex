//! Certificate compression and decompression support
//!
//! This crate supports compression and decompression everywhere
//! certificates are used, in accordance with [RFC8879][rfc8879].
//!
//! Note that this is only supported for TLS1.3 connections.
//!
//! # Getting started
//!
//! Build this crate with the `brotli` and/or `zlib` crate features.  This
//! adds dependencies on these crates.  They are used by default if enabled.
//!
//! We especially recommend `brotli` as it has the widest deployment so far.
//!
//! # Custom compression/decompression implementations
//!
//! 1. Implement the [`CertCompressor`] and/or [`CertDecompressor`] traits
//! 2. Provide those to:
//!   - [`ClientConfig::cert_compressors`][cc_cc] or [`ServerConfig::cert_compressors`][sc_cc].
//!   - [`ClientConfig::cert_decompressors`][cc_cd] or [`ServerConfig::cert_decompressors`][sc_cd].
//!
//! These are used in these circumstances:
//!
//! | Peer | Client authentication | Server authentication |
//! | ---- | --------------------- | --------------------- |
//! | *Client* | [`ClientConfig::cert_compressors`][cc_cc] | [`ClientConfig::cert_decompressors`][cc_cd] |
//! | *Server* | [`ServerConfig::cert_decompressors`][sc_cd] | [`ServerConfig::cert_compressors`][sc_cc] |
//!
//! [rfc8879]: https://datatracker.ietf.org/doc/html/rfc8879
//! [cc_cc]: crate::ClientConfig::cert_compressors
//! [sc_cc]: crate::ServerConfig::cert_compressors
//! [cc_cd]: crate::ClientConfig::cert_decompressors
//! [sc_cd]: crate::ServerConfig::cert_decompressors

#[cfg(feature = "std")]
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::fmt::Debug;
#[cfg(feature = "std")]
use std::sync::Mutex;

use crate::enums::CertificateCompressionAlgorithm;
use crate::msgs::base::{Payload, PayloadU24};
use crate::msgs::codec::Codec;
use crate::msgs::handshake::{CertificatePayloadTls13, CompressedCertificatePayload};
use crate::sync::Arc;

/// Returns the supported `CertDecompressor` implementations enabled
/// by crate features.
pub fn default_cert_decompressors() -> &'static [&'static dyn CertDecompressor] {
    &[
        #[cfg(feature = "brotli")]
        BROTLI_DECOMPRESSOR,
        #[cfg(feature = "zlib")]
        ZLIB_DECOMPRESSOR,
    ]
}

/// An available certificate decompression algorithm.
pub trait CertDecompressor: Debug + Send + Sync {
    /// Decompress `input`, writing the result to `output`.
    ///
    /// `output` is sized to match the declared length of the decompressed data.
    ///
    /// `Err(DecompressionFailed)` should be returned if decompression produces more, or fewer
    /// bytes than fit in `output`, or if the `input` is in any way malformed.
    fn decompress(&self, input: &[u8], output: &mut [u8]) -> Result<(), DecompressionFailed>;

    /// Which algorithm this decompressor handles.
    fn algorithm(&self) -> CertificateCompressionAlgorithm;
}

/// Returns the supported `CertCompressor` implementations enabled
/// by crate features.
pub fn default_cert_compressors() -> &'static [&'static dyn CertCompressor] {
    &[
        #[cfg(feature = "brotli")]
        BROTLI_COMPRESSOR,
        #[cfg(feature = "zlib")]
        ZLIB_COMPRESSOR,
    ]
}

/// An available certificate compression algorithm.
pub trait CertCompressor: Debug + Send + Sync {
    /// Compress `input`, returning the result.
    ///
    /// `input` is consumed by this function so (if the underlying implementation
    /// supports it) the compression can be performed in-place.
    ///
    /// `level` is a hint as to how much effort to expend on the compression.
    ///
    /// `Err(CompressionFailed)` may be returned for any reason.
    fn compress(
        &self,
        input: Vec<u8>,
        level: CompressionLevel,
    ) -> Result<Vec<u8>, CompressionFailed>;

    /// Which algorithm this compressor handles.
    fn algorithm(&self) -> CertificateCompressionAlgorithm;
}

/// A hint for how many resources to dedicate to a compression.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CompressionLevel {
    /// This compression is happening interactively during a handshake.
    ///
    /// Implementations may wish to choose a conservative compression level.
    Interactive,

    /// The compression may be amortized over many connections.
    ///
    /// Implementations may wish to choose an aggressive compression level.
    Amortized,
}

/// A content-less error for when `CertDecompressor::decompress` fails.
#[derive(Debug)]
pub struct DecompressionFailed;

/// A content-less error for when `CertCompressor::compress` fails.
#[derive(Debug)]
pub struct CompressionFailed;

#[cfg(feature = "zlib")]
mod feat_zlib_rs {
    use zlib_rs::c_api::Z_BEST_COMPRESSION;
    use zlib_rs::{ReturnCode, deflate, inflate};

    use super::*;

    /// A certificate decompressor for the Zlib algorithm using the `zlib-rs` crate.
    pub const ZLIB_DECOMPRESSOR: &dyn CertDecompressor = &ZlibRsDecompressor;

    #[derive(Debug)]
    struct ZlibRsDecompressor;

    impl CertDecompressor for ZlibRsDecompressor {
        fn decompress(&self, input: &[u8], output: &mut [u8]) -> Result<(), DecompressionFailed> {
            let output_len = output.len();
            match inflate::uncompress_slice(output, input, inflate::InflateConfig::default()) {
                (output_filled, ReturnCode::Ok) if output_filled.len() == output_len => Ok(()),
                (_, _) => Err(DecompressionFailed),
            }
        }

        fn algorithm(&self) -> CertificateCompressionAlgorithm {
            CertificateCompressionAlgorithm::Zlib
        }
    }

    /// A certificate compressor for the Zlib algorithm using the `zlib-rs` crate.
    pub const ZLIB_COMPRESSOR: &dyn CertCompressor = &ZlibRsCompressor;

    #[derive(Debug)]
    struct ZlibRsCompressor;

    impl CertCompressor for ZlibRsCompressor {
        fn compress(
            &self,
            input: Vec<u8>,
            level: CompressionLevel,
        ) -> Result<Vec<u8>, CompressionFailed> {
            let mut output = alloc::vec![0u8; deflate::compress_bound(input.len())];
            let config = match level {
                CompressionLevel::Interactive => deflate::DeflateConfig::default(),
                CompressionLevel::Amortized => deflate::DeflateConfig::new(Z_BEST_COMPRESSION),
            };
            let (output_filled, rc) = deflate::compress_slice(&mut output, &input, config);
            if rc != ReturnCode::Ok {
                return Err(CompressionFailed);
            }

            let used = output_filled.len();
            output.truncate(used);
            Ok(output)
        }

        fn algorithm(&self) -> CertificateCompressionAlgorithm {
            CertificateCompressionAlgorithm::Zlib
        }
    }
}

#[cfg(feature = "zlib")]
pub use feat_zlib_rs::{ZLIB_COMPRESSOR, ZLIB_DECOMPRESSOR};

#[cfg(feature = "brotli")]
mod feat_brotli {
    use std::io::{Cursor, Write};

    use super::*;

    /// A certificate decompressor for the brotli algorithm using the `brotli` crate.
    pub const BROTLI_DECOMPRESSOR: &dyn CertDecompressor = &BrotliDecompressor;

    #[derive(Debug)]
    struct BrotliDecompressor;

    impl CertDecompressor for BrotliDecompressor {
        fn decompress(&self, input: &[u8], output: &mut [u8]) -> Result<(), DecompressionFailed> {
            let mut in_cursor = Cursor::new(input);
            let mut out_cursor = Cursor::new(output);

            brotli::BrotliDecompress(&mut in_cursor, &mut out_cursor)
                .map_err(|_| DecompressionFailed)?;

            if out_cursor.position() as usize != out_cursor.into_inner().len() {
                return Err(DecompressionFailed);
            }

            Ok(())
        }

        fn algorithm(&self) -> CertificateCompressionAlgorithm {
            CertificateCompressionAlgorithm::Brotli
        }
    }

    /// A certificate compressor for the brotli algorithm using the `brotli` crate.
    pub const BROTLI_COMPRESSOR: &dyn CertCompressor = &BrotliCompressor;

    #[derive(Debug)]
    struct BrotliCompressor;

    impl CertCompressor for BrotliCompressor {
        fn compress(
            &self,
            input: Vec<u8>,
            level: CompressionLevel,
        ) -> Result<Vec<u8>, CompressionFailed> {
            let quality = match level {
                CompressionLevel::Interactive => QUALITY_FAST,
                CompressionLevel::Amortized => QUALITY_SLOW,
            };
            let output = Cursor::new(Vec::with_capacity(input.len() / 2));
            let mut compressor = brotli::CompressorWriter::new(output, BUFFER_SIZE, quality, LGWIN);
            compressor
                .write_all(&input)
                .map_err(|_| CompressionFailed)?;
            Ok(compressor.into_inner().into_inner())
        }

        fn algorithm(&self) -> CertificateCompressionAlgorithm {
            CertificateCompressionAlgorithm::Brotli
        }
    }

    /// Brotli buffer size.
    ///
    /// Chosen based on brotli `examples/compress.rs`.
    const BUFFER_SIZE: usize = 4096;

    /// This is the default lgwin parameter, see `BrotliEncoderInitParams()`
    const LGWIN: u32 = 22;

    /// Compression quality we use for interactive compressions.
    /// See <https://blog.cloudflare.com/results-experimenting-brotli> for data.
    const QUALITY_FAST: u32 = 4;

    /// Compression quality we use for offline compressions (the maximum).
    const QUALITY_SLOW: u32 = 11;
}

#[cfg(feature = "brotli")]
pub use feat_brotli::{BROTLI_COMPRESSOR, BROTLI_DECOMPRESSOR};

/// An LRU cache for compressions.
///
/// The prospect of being able to reuse a given compression for many connections
/// means we can afford to spend more time on that compression (by passing
/// `CompressionLevel::Amortized` to the compressor).
#[derive(Debug)]
pub enum CompressionCache {
    /// No caching happens, and compression happens each time using
    /// `CompressionLevel::Interactive`.
    Disabled,

    /// Compressions are stored in an LRU cache.
    #[cfg(feature = "std")]
    Enabled(CompressionCacheInner),
}

/// Innards of an enabled CompressionCache.
///
/// You cannot make one of these directly. Use [`CompressionCache::new`].
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct CompressionCacheInner {
    /// Maximum size of underlying storage.
    size: usize,

    /// LRU-order entries.
    ///
    /// First is least-used, last is most-used.
    entries: Mutex<VecDeque<Arc<CompressionCacheEntry>>>,
}

impl CompressionCache {
    /// Make a `CompressionCache` that stores up to `size` compressed
    /// certificate messages.
    #[cfg(feature = "std")]
    pub fn new(size: usize) -> Self {
        if size == 0 {
            return Self::Disabled;
        }

        Self::Enabled(CompressionCacheInner {
            size,
            entries: Mutex::new(VecDeque::with_capacity(size)),
        })
    }

    /// Return a `CompressionCacheEntry`, which is an owning
    /// wrapper for a `CompressedCertificatePayload`.
    ///
    /// `compressor` is the compression function we have negotiated.
    /// `original` is the uncompressed certificate message.
    pub(crate) fn compression_for(
        &self,
        compressor: &dyn CertCompressor,
        original: &CertificatePayloadTls13<'_>,
    ) -> Result<Arc<CompressionCacheEntry>, CompressionFailed> {
        match self {
            Self::Disabled => Self::uncached_compression(compressor, original),

            #[cfg(feature = "std")]
            Self::Enabled(_) => self.compression_for_impl(compressor, original),
        }
    }

    #[cfg(feature = "std")]
    fn compression_for_impl(
        &self,
        compressor: &dyn CertCompressor,
        original: &CertificatePayloadTls13<'_>,
    ) -> Result<Arc<CompressionCacheEntry>, CompressionFailed> {
        let (max_size, entries) = match self {
            Self::Enabled(CompressionCacheInner { size, entries }) => (*size, entries),
            _ => unreachable!(),
        };

        // context is a per-connection quantity, and included in the compressed data.
        // it is not suitable for inclusion in the cache.
        if !original.context.0.is_empty() {
            return Self::uncached_compression(compressor, original);
        }

        // cache probe:
        let encoding = original.get_encoding();
        let algorithm = compressor.algorithm();

        let mut cache = entries
            .lock()
            .map_err(|_| CompressionFailed)?;
        for (i, item) in cache.iter().enumerate() {
            if item.algorithm == algorithm && item.original == encoding {
                // this item is now MRU
                let item = cache.remove(i).unwrap();
                cache.push_back(item.clone());
                return Ok(item);
            }
        }
        drop(cache);

        // do compression:
        let uncompressed_len = encoding.len() as u32;
        let compressed = compressor.compress(encoding.clone(), CompressionLevel::Amortized)?;
        let new_entry = Arc::new(CompressionCacheEntry {
            algorithm,
            original: encoding,
            compressed: CompressedCertificatePayload {
                alg: algorithm,
                uncompressed_len,
                compressed: PayloadU24(Payload::new(compressed)),
            },
        });

        // insert into cache
        let mut cache = entries
            .lock()
            .map_err(|_| CompressionFailed)?;
        if cache.len() == max_size {
            cache.pop_front();
        }
        cache.push_back(new_entry.clone());
        Ok(new_entry)
    }

    /// Compress `original` using `compressor` at `Interactive` level.
    fn uncached_compression(
        compressor: &dyn CertCompressor,
        original: &CertificatePayloadTls13<'_>,
    ) -> Result<Arc<CompressionCacheEntry>, CompressionFailed> {
        let algorithm = compressor.algorithm();
        let encoding = original.get_encoding();
        let uncompressed_len = encoding.len() as u32;
        let compressed = compressor.compress(encoding, CompressionLevel::Interactive)?;

        // this `CompressionCacheEntry` in fact never makes it into the cache, so
        // `original` is left empty
        Ok(Arc::new(CompressionCacheEntry {
            algorithm,
            original: Vec::new(),
            compressed: CompressedCertificatePayload {
                alg: algorithm,
                uncompressed_len,
                compressed: PayloadU24(Payload::new(compressed)),
            },
        }))
    }
}

impl Default for CompressionCache {
    fn default() -> Self {
        #[cfg(feature = "std")]
        {
            // 4 entries allows 2 certificate chains times 2 compression algorithms
            Self::new(4)
        }

        #[cfg(not(feature = "std"))]
        {
            Self::Disabled
        }
    }
}

#[cfg_attr(not(feature = "std"), allow(dead_code))]
#[derive(Debug)]
pub(crate) struct CompressionCacheEntry {
    // cache key is algorithm + original:
    algorithm: CertificateCompressionAlgorithm,
    original: Vec<u8>,

    // cache value is compression result:
    compressed: CompressedCertificatePayload<'static>,
}

impl CompressionCacheEntry {
    pub(crate) fn compressed_cert_payload(&self) -> CompressedCertificatePayload<'_> {
        self.compressed.as_borrowed()
    }
}

#[cfg(all(test, any(feature = "brotli", feature = "zlib")))]
mod tests {
    use std::{println, vec};

    use super::*;

    #[test]
    #[cfg(feature = "zlib")]
    fn test_zlib() {
        test_compressor(ZLIB_COMPRESSOR, ZLIB_DECOMPRESSOR);
    }

    #[test]
    #[cfg(feature = "brotli")]
    fn test_brotli() {
        test_compressor(BROTLI_COMPRESSOR, BROTLI_DECOMPRESSOR);
    }

    fn test_compressor(comp: &dyn CertCompressor, decomp: &dyn CertDecompressor) {
        assert_eq!(comp.algorithm(), decomp.algorithm());
        for sz in [16, 64, 512, 2048, 8192, 16384] {
            test_trivial_pairwise(comp, decomp, sz);
        }
        test_decompress_wrong_len(comp, decomp);
        test_decompress_garbage(decomp);
    }

    fn test_trivial_pairwise(
        comp: &dyn CertCompressor,
        decomp: &dyn CertDecompressor,
        plain_len: usize,
    ) {
        let original = vec![0u8; plain_len];

        for level in [CompressionLevel::Interactive, CompressionLevel::Amortized] {
            let compressed = comp
                .compress(original.clone(), level)
                .unwrap();
            println!(
                "{:?} compressed trivial {} -> {} using {:?} level",
                comp.algorithm(),
                original.len(),
                compressed.len(),
                level
            );
            let mut recovered = vec![0xffu8; plain_len];
            decomp
                .decompress(&compressed, &mut recovered)
                .unwrap();
            assert_eq!(original, recovered);
        }
    }

    fn test_decompress_wrong_len(comp: &dyn CertCompressor, decomp: &dyn CertDecompressor) {
        let original = vec![0u8; 2048];
        let compressed = comp
            .compress(original.clone(), CompressionLevel::Interactive)
            .unwrap();
        println!("{compressed:?}");

        // too big
        let mut recovered = vec![0xffu8; original.len() + 1];
        decomp
            .decompress(&compressed, &mut recovered)
            .unwrap_err();

        // too small
        let mut recovered = vec![0xffu8; original.len() - 1];
        decomp
            .decompress(&compressed, &mut recovered)
            .unwrap_err();
    }

    fn test_decompress_garbage(decomp: &dyn CertDecompressor) {
        let junk = [0u8; 1024];
        let mut recovered = vec![0u8; 512];
        decomp
            .decompress(&junk, &mut recovered)
            .unwrap_err();
    }

    #[test]
    #[cfg(all(feature = "brotli", feature = "zlib"))]
    fn test_cache_evicts_lru() {
        use core::sync::atomic::{AtomicBool, Ordering};

        use pki_types::CertificateDer;

        let cache = CompressionCache::default();

        let cert = CertificateDer::from(vec![1]);

        let cert1 = CertificatePayloadTls13::new([&cert].into_iter(), Some(b"1"));
        let cert2 = CertificatePayloadTls13::new([&cert].into_iter(), Some(b"2"));
        let cert3 = CertificatePayloadTls13::new([&cert].into_iter(), Some(b"3"));
        let cert4 = CertificatePayloadTls13::new([&cert].into_iter(), Some(b"4"));

        // insert zlib (1), (2), (3), (4)

        cache
            .compression_for(
                &RequireCompress(ZLIB_COMPRESSOR, AtomicBool::default(), true),
                &cert1,
            )
            .unwrap();
        cache
            .compression_for(
                &RequireCompress(ZLIB_COMPRESSOR, AtomicBool::default(), true),
                &cert2,
            )
            .unwrap();
        cache
            .compression_for(
                &RequireCompress(ZLIB_COMPRESSOR, AtomicBool::default(), true),
                &cert3,
            )
            .unwrap();
        cache
            .compression_for(
                &RequireCompress(ZLIB_COMPRESSOR, AtomicBool::default(), true),
                &cert4,
            )
            .unwrap();

        // -- now full

        // insert brotli (1) evicts zlib (1)
        cache
            .compression_for(
                &RequireCompress(BROTLI_COMPRESSOR, AtomicBool::default(), true),
                &cert4,
            )
            .unwrap();

        // now zlib (2), (3), (4) and brotli (4) exist
        cache
            .compression_for(
                &RequireCompress(ZLIB_COMPRESSOR, AtomicBool::default(), false),
                &cert2,
            )
            .unwrap();
        cache
            .compression_for(
                &RequireCompress(ZLIB_COMPRESSOR, AtomicBool::default(), false),
                &cert3,
            )
            .unwrap();
        cache
            .compression_for(
                &RequireCompress(ZLIB_COMPRESSOR, AtomicBool::default(), false),
                &cert4,
            )
            .unwrap();
        cache
            .compression_for(
                &RequireCompress(BROTLI_COMPRESSOR, AtomicBool::default(), false),
                &cert4,
            )
            .unwrap();

        // insert zlib (1) requires re-compression & evicts zlib (2)
        cache
            .compression_for(
                &RequireCompress(ZLIB_COMPRESSOR, AtomicBool::default(), true),
                &cert1,
            )
            .unwrap();

        // now zlib (1), (3), (4) and brotli (4) exist
        // query zlib (4), (3), (1) to demonstrate LRU tracks usage rather than insertion
        cache
            .compression_for(
                &RequireCompress(ZLIB_COMPRESSOR, AtomicBool::default(), false),
                &cert4,
            )
            .unwrap();
        cache
            .compression_for(
                &RequireCompress(ZLIB_COMPRESSOR, AtomicBool::default(), false),
                &cert3,
            )
            .unwrap();
        cache
            .compression_for(
                &RequireCompress(ZLIB_COMPRESSOR, AtomicBool::default(), false),
                &cert1,
            )
            .unwrap();

        // now brotli (4), zlib (4), (3), (1)
        // insert brotli (1) evicting brotli (4)
        cache
            .compression_for(
                &RequireCompress(BROTLI_COMPRESSOR, AtomicBool::default(), true),
                &cert1,
            )
            .unwrap();

        // verify brotli (4) disappeared
        cache
            .compression_for(
                &RequireCompress(BROTLI_COMPRESSOR, AtomicBool::default(), true),
                &cert4,
            )
            .unwrap();

        #[derive(Debug)]
        struct RequireCompress(&'static dyn CertCompressor, AtomicBool, bool);

        impl CertCompressor for RequireCompress {
            fn compress(
                &self,
                input: Vec<u8>,
                level: CompressionLevel,
            ) -> Result<Vec<u8>, CompressionFailed> {
                self.1.store(true, Ordering::SeqCst);
                self.0.compress(input, level)
            }

            fn algorithm(&self) -> CertificateCompressionAlgorithm {
                self.0.algorithm()
            }
        }

        impl Drop for RequireCompress {
            fn drop(&mut self) {
                assert_eq!(self.1.load(Ordering::SeqCst), self.2);
            }
        }
    }
}
