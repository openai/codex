//! This module contains functionality for compression.

use crate::alloc::vec;
use crate::alloc::vec::Vec;

mod buffer;
pub mod core;
mod stored;
pub mod stream;
mod zlib;
use self::core::*;

/// How much processing the compressor should do to compress the data.
/// `NoCompression` and `Bestspeed` have special meanings, the other levels determine the number
/// of checks for matches in the hash chains and whether to use lazy or greedy parsing.
#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CompressionLevel {
    /// Don't do any compression, only output uncompressed blocks.
    NoCompression = 0,
    /// Fast compression. Uses a special compression routine that is optimized for speed.
    BestSpeed = 1,
    /// Slow/high compression. Do a lot of checks to try to find good matches.
    BestCompression = 9,
    /// Even more checks, can be very slow.
    UberCompression = 10,
    /// Default compromise between speed and compression.
    DefaultLevel = 6,
    /// Use the default compression level.
    DefaultCompression = -1,
}

// Missing safe rust analogue (this and mem-to-mem are quite similar)
/*
fn tdefl_compress(
    d: Option<&mut CompressorOxide>,
    in_buf: *const c_void,
    in_size: Option<&mut usize>,
    out_buf: *mut c_void,
    out_size: Option<&mut usize>,
    flush: TDEFLFlush,
) -> TDEFLStatus {
    let res = match d {
        None => {
            in_size.map(|size| *size = 0);
            out_size.map(|size| *size = 0);
            (TDEFLStatus::BadParam, 0, 0)
        },
        Some(compressor) => {
            let callback_res = CallbackOxide::new(
                compressor.callback_func.clone(),
                in_buf,
                in_size,
                out_buf,
                out_size,
            );

            if let Ok(mut callback) = callback_res {
                let res = compress(compressor, &mut callback, flush);
                callback.update_size(Some(res.1), Some(res.2));
                res
            } else {
                (TDEFLStatus::BadParam, 0, 0)
            }
        }
    };
    res.0
}*/

// Missing safe rust analogue
/*
fn tdefl_init(
    d: Option<&mut CompressorOxide>,
    put_buf_func: PutBufFuncPtr,
    put_buf_user: *mut c_void,
    flags: c_int,
) -> TDEFLStatus {
    if let Some(d) = d {
        *d = CompressorOxide::new(
            put_buf_func.map(|func|
                CallbackFunc { put_buf_func: func, put_buf_user: put_buf_user }
            ),
            flags as u32,
        );
        TDEFLStatus::Okay
    } else {
        TDEFLStatus::BadParam
    }
}*/

// Missing safe rust analogue (though maybe best served by flate2 front-end instead)
/*
fn tdefl_compress_mem_to_output(
    buf: *const c_void,
    buf_len: usize,
    put_buf_func: PutBufFuncPtr,
    put_buf_user: *mut c_void,
    flags: c_int,
) -> bool*/

// Missing safe Rust analogue
/*
fn tdefl_compress_mem_to_mem(
    out_buf: *mut c_void,
    out_buf_len: usize,
    src_buf: *const c_void,
    src_buf_len: usize,
    flags: c_int,
) -> usize*/

/// Compress the input data to a vector, using the specified compression level (0-10).
pub fn compress_to_vec(input: &[u8], level: u8) -> Vec<u8> {
    compress_to_vec_inner(input, level, 0, 0)
}

/// Compress the input data to a vector, using the specified compression level (0-10), and with a
/// zlib wrapper.
pub fn compress_to_vec_zlib(input: &[u8], level: u8) -> Vec<u8> {
    compress_to_vec_inner(input, level, 1, 0)
}

/// Simple function to compress data to a vec.
fn compress_to_vec_inner(mut input: &[u8], level: u8, window_bits: i32, strategy: i32) -> Vec<u8> {
    // The comp flags function sets the zlib flag if the window_bits parameter is > 0.
    let flags = create_comp_flags_from_zip_params(level.into(), window_bits, strategy);
    let mut compressor = CompressorOxide::new(flags);
    let mut output = vec![0; ::core::cmp::max(input.len() / 2, 2)];

    let mut out_pos = 0;
    loop {
        let (status, bytes_in, bytes_out) = compress(
            &mut compressor,
            input,
            &mut output[out_pos..],
            TDEFLFlush::Finish,
        );
        out_pos += bytes_out;

        match status {
            TDEFLStatus::Done => {
                output.truncate(out_pos);
                break;
            }
            TDEFLStatus::Okay if bytes_in <= input.len() => {
                input = &input[bytes_in..];

                // We need more space, so resize the vector.
                if output.len().saturating_sub(out_pos) < 30 {
                    output.resize(output.len() * 2, 0)
                }
            }
            // Not supposed to happen unless there is a bug.
            _ => panic!("Bug! Unexpectedly failed to compress!"),
        }
    }

    output
}

#[cfg(test)]
mod test {
    use super::{compress_to_vec, compress_to_vec_inner, CompressionStrategy};
    use crate::inflate::decompress_to_vec;
    use alloc::vec;

    /// Test deflate example.
    ///
    /// Check if the encoder produces the same code as the example given by Mark Adler here:
    /// https://stackoverflow.com/questions/17398931/deflate-encoding-with-static-huffman-codes/17415203
    #[test]
    fn compress_small() {
        let test_data = b"Deflate late";
        let check = [
            0x73, 0x49, 0x4d, 0xcb, 0x49, 0x2c, 0x49, 0x55, 0x00, 0x11, 0x00,
        ];

        let res = compress_to_vec(test_data, 1);
        assert_eq!(&check[..], res.as_slice());

        let res = compress_to_vec(test_data, 9);
        assert_eq!(&check[..], res.as_slice());
    }

    #[test]
    fn compress_huff_only() {
        let test_data = b"Deflate late";

        let res = compress_to_vec_inner(test_data, 1, 0, CompressionStrategy::HuffmanOnly as i32);
        let d = decompress_to_vec(res.as_slice()).expect("Failed to decompress!");
        assert_eq!(test_data, d.as_slice());
    }

    #[test]
    fn compress_rle() {
        let test_data = b"Deflate late";

        let res = compress_to_vec_inner(test_data, 1, 0, CompressionStrategy::RLE as i32);
        let d = decompress_to_vec(res.as_slice()).expect("Failed to decompress!");
        assert_eq!(test_data, d.as_slice());
    }

    /// Test that a raw block compresses fine.
    #[test]
    fn compress_raw() {
        let text = b"Hello, zlib!";
        let encoded = {
            let len = text.len();
            let notlen = !len;
            let mut encoded = vec![
                1,
                len as u8,
                (len >> 8) as u8,
                notlen as u8,
                (notlen >> 8) as u8,
            ];
            encoded.extend_from_slice(&text[..]);
            encoded
        };

        let res = compress_to_vec(text, 0);
        assert_eq!(encoded, res.as_slice());
    }

    #[test]
    fn short() {
        let test_data = [10, 10, 10, 10, 10, 55];
        let c = compress_to_vec(&test_data, 9);

        let d = decompress_to_vec(c.as_slice()).expect("Failed to decompress!");
        assert_eq!(&test_data, d.as_slice());
        // Check that a static block is used here, rather than a raw block
        // , so the data is actually compressed.
        // (The optimal compressed length would be 5, but neither miniz nor zlib manages that either
        // as neither checks matches against the byte at index 0.)
        assert!(c.len() <= 6);
    }
}
