//! Streaming compression functionality.

use alloc::boxed::Box;
use core::convert::TryInto;
use core::{cmp, mem};

use super::super::*;
use super::deflate_flags::*;
use super::CompressionLevel;
use crate::deflate::buffer::{
    update_hash, HashBuffers, LocalBuf, LZ_CODE_BUF_MASK, LZ_CODE_BUF_SIZE, LZ_DICT_FULL_SIZE,
    LZ_HASH_BITS, LZ_HASH_SHIFT, LZ_HASH_SIZE, OUT_BUF_SIZE,
};
use crate::deflate::stored::compress_stored;
use crate::deflate::zlib;
use crate::shared::{update_adler32, HUFFMAN_LENGTH_ORDER, MZ_ADLER32_INIT};
use crate::DataFormat;

// Currently not bubbled up outside this module, so can fill in with more
// context eventually if needed.
type Result<T, E = Error> = core::result::Result<T, E>;
pub(crate) struct Error {}

pub(crate) const MAX_PROBES_MASK: u32 = 0xFFF;

const MAX_SUPPORTED_HUFF_CODESIZE: usize = 15;

// Length code for length values - 256.
// We use an offset to help with bound check avoidance as we can mask values to 32
// and it also saves some memory as we can use a u8 instead of a u16.
// Conventiently our table is large enough that we can get away with using an
// offset of 256 which results in very efficient code.
const LEN_SYM: [u8; 256] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 13, 13, 14, 14, 14, 14, 15, 15,
    15, 15, 16, 16, 16, 16, 17, 17, 17, 17, 17, 17, 17, 17, 18, 18, 18, 18, 18, 18, 18, 18, 19, 19,
    19, 19, 19, 19, 19, 19, 20, 20, 20, 20, 20, 20, 20, 20, 21, 21, 21, 21, 21, 21, 21, 21, 21, 21,
    21, 21, 21, 21, 21, 21, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 23, 23,
    23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 23, 24, 24, 24, 24, 24, 24, 24, 24, 24, 24,
    24, 24, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25,
    25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 25, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26,
    26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 27, 27,
    27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27,
    27, 27, 27, 27, 27, 27, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
    28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 29,
];

const LEN_SYM_OFFSET: usize = 256;

/// Number of extra bits for length values.
#[rustfmt::skip]
const LEN_EXTRA: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1,
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 0
];

/// Distance codes for distances smaller than 512.
#[rustfmt::skip]
const SMALL_DIST_SYM: [u8; 512] = [
     0,  1,  2,  3,  4,  4,  5,  5,  6,  6,  6,  6,  7,  7,  7,  7,
     8,  8,  8,  8,  8,  8,  8,  8,  9,  9,  9,  9,  9,  9,  9,  9,
    10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10,
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
    12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12,
    12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12,
    13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
    13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
    14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14,
    14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14,
    14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14,
    14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16, 16,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17,
    17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 17
];

/// Number of extra bits for distances smaller than 512.
#[rustfmt::skip]
const SMALL_DIST_EXTRA: [u8; 512] = [
    0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7
];

/// Base values to calculate distances above 512.
#[rustfmt::skip]
const LARGE_DIST_SYM: [u8; 128] = [
     0,  0, 18, 19, 20, 20, 21, 21, 22, 22, 22, 22, 23, 23, 23, 23,
    24, 24, 24, 24, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 25,
    26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26, 26,
    27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27, 27,
    28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
    28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
    29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29,
    29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29, 29
];

/// Number of extra bits distances above 512.
#[rustfmt::skip]
const LARGE_DIST_EXTRA: [u8; 128] = [
     0,  0,  8,  8,  9,  9,  9,  9, 10, 10, 10, 10, 10, 10, 10, 10,
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
    12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12,
    12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12,
    13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
    13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
    13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13,
    13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13
];

#[rustfmt::skip]
const BITMASKS: [u32; 17] = [
    0x0000, 0x0001, 0x0003, 0x0007, 0x000F, 0x001F, 0x003F, 0x007F, 0x00FF,
    0x01FF, 0x03FF, 0x07FF, 0x0FFF, 0x1FFF, 0x3FFF, 0x7FFF, 0xFFFF
];

/// The maximum number of checks for matches in the hash table the compressor will make for each
/// compression level.
pub(crate) const NUM_PROBES: [u16; 11] = [0, 1, 6, 32, 16, 32, 128, 256, 512, 768, 1500];

#[derive(Copy, Clone)]
struct SymFreq {
    key: u16,
    sym_index: u16,
}

pub mod deflate_flags {
    /// Whether to use a zlib wrapper.
    pub const TDEFL_WRITE_ZLIB_HEADER: u32 = 0x0000_1000;
    /// Should we compute the adler32 checksum.
    pub const TDEFL_COMPUTE_ADLER32: u32 = 0x0000_2000;
    /// Should we use greedy parsing (as opposed to lazy parsing where look ahead one or more
    /// bytes to check for better matches.)
    pub const TDEFL_GREEDY_PARSING_FLAG: u32 = 0x0000_4000;
    /// Used in miniz to skip zero-initializing hash and dict. We don't do this here, so
    /// this flag is ignored.
    pub const TDEFL_NONDETERMINISTIC_PARSING_FLAG: u32 = 0x0000_8000;
    /// Only look for matches with a distance of 0.
    pub const TDEFL_RLE_MATCHES: u32 = 0x0001_0000;
    /// Only use matches that are at least 6 bytes long.
    pub const TDEFL_FILTER_MATCHES: u32 = 0x0002_0000;
    /// Force the compressor to only output static blocks. (Blocks using the default huffman codes
    /// specified in the deflate specification.)
    pub const TDEFL_FORCE_ALL_STATIC_BLOCKS: u32 = 0x0004_0000;
    /// Force the compressor to only output raw/uncompressed blocks.
    pub const TDEFL_FORCE_ALL_RAW_BLOCKS: u32 = 0x0008_0000;
}

/// Strategy setting for compression.
///
/// The non-default settings offer some special-case compression variants.
#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CompressionStrategy {
    /// Don't use any of the special strategies.
    Default = 0,
    /// Only use matches that are at least 5 bytes long.
    Filtered = 1,
    /// Don't look for matches, only huffman encode the literals.
    HuffmanOnly = 2,
    /// Only look for matches with a distance of 1, i.e do run-length encoding only.
    RLE = 3,
    /// Only use static/fixed blocks. (Blocks using the default huffman codes
    /// specified in the deflate specification.)
    Fixed = 4,
}

impl From<CompressionStrategy> for i32 {
    #[inline(always)]
    fn from(value: CompressionStrategy) -> Self {
        value as i32
    }
}

/// A list of deflate flush types.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TDEFLFlush {
    /// Normal operation.
    ///
    /// Compress as much as there is space for, and then return waiting for more input.
    None = 0,

    /// Try to flush all the current data and output an empty raw block.
    Sync = 2,

    /// Same as [`Sync`][Self::Sync], but reset the dictionary so that the following data does not
    /// depend on previous data.
    Full = 3,

    /// Try to flush everything and end the deflate stream.
    ///
    /// On success this will yield a [`TDEFLStatus::Done`] return status.
    Finish = 4,
}

impl From<MZFlush> for TDEFLFlush {
    fn from(flush: MZFlush) -> Self {
        match flush {
            MZFlush::None => TDEFLFlush::None,
            MZFlush::Sync => TDEFLFlush::Sync,
            MZFlush::Full => TDEFLFlush::Full,
            MZFlush::Finish => TDEFLFlush::Finish,
            _ => TDEFLFlush::None, // TODO: ??? What to do ???
        }
    }
}

impl TDEFLFlush {
    pub const fn new(flush: i32) -> Result<Self, MZError> {
        match flush {
            0 => Ok(TDEFLFlush::None),
            2 => Ok(TDEFLFlush::Sync),
            3 => Ok(TDEFLFlush::Full),
            4 => Ok(TDEFLFlush::Finish),
            _ => Err(MZError::Param),
        }
    }
}

/// Return status of compression.
#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TDEFLStatus {
    /// Usage error.
    ///
    /// This indicates that either the [`CompressorOxide`] experienced a previous error, or the
    /// stream has already been [`TDEFLFlush::Finish`]'d.
    BadParam = -2,

    /// Error putting data into output buffer.
    ///
    /// This usually indicates a too-small buffer.
    PutBufFailed = -1,

    /// Compression succeeded normally.
    Okay = 0,

    /// Compression succeeded and the deflate stream was ended.
    ///
    /// This is the result of calling compression with [`TDEFLFlush::Finish`].
    Done = 1,
}

const MAX_HUFF_SYMBOLS: usize = 288;
/// Size of hash chain for fast compression mode.
const LEVEL1_HASH_SIZE_MASK: u32 = 4095;
/// The number of huffman tables used by the compressor.
/// Literal/length, Distances and Length of the huffman codes for the other two tables.
const MAX_HUFF_TABLES: usize = 3;
/// Literal/length codes
const MAX_HUFF_SYMBOLS_0: usize = 288;
/// Distance codes.
const MAX_HUFF_SYMBOLS_1: usize = 32;
/// Huffman length values.
const MAX_HUFF_SYMBOLS_2: usize = 19;
/// Size of the chained hash table.
pub(crate) const LZ_DICT_SIZE: usize = 32_768;
/// Mask used when stepping through the hash chains.
pub(crate) const LZ_DICT_SIZE_MASK: usize = (LZ_DICT_SIZE as u32 - 1) as usize;
/// The minimum length of a match.
pub(crate) const MIN_MATCH_LEN: u8 = 3;
/// The maximum length of a match.
pub(crate) const MAX_MATCH_LEN: usize = 258;

pub(crate) const DEFAULT_FLAGS: u32 = NUM_PROBES[4] as u32 | TDEFL_WRITE_ZLIB_HEADER;

#[cfg(test)]
#[inline]
fn write_u16_le(val: u16, slice: &mut [u8], pos: usize) {
    slice[pos] = val as u8;
    slice[pos + 1] = (val >> 8) as u8;
}

// Read the two bytes starting at pos and interpret them as an u16.
#[inline(always)]
const fn read_u16_le<const N: usize>(slice: &[u8; N], pos: usize) -> u16 {
    // The compiler is smart enough to optimize this into an unaligned load.
    slice[pos] as u16 | ((slice[pos + 1] as u16) << 8)
}

/// Main compression struct.
pub struct CompressorOxide {
    pub(crate) lz: LZOxide,
    pub(crate) params: ParamsOxide,
    /// Put HuffmanOxide on the heap with default trick to avoid
    /// excessive stack copies.
    pub(crate) huff: Box<HuffmanOxide>,
    pub(crate) dict: DictOxide,
}

impl CompressorOxide {
    /// Create a new `CompressorOxide` with the given flags.
    ///
    /// # Notes
    /// This function may be changed to take different parameters in the future.
    pub fn new(flags: u32) -> Self {
        CompressorOxide {
            lz: LZOxide::new(),
            params: ParamsOxide::new(flags),
            huff: Box::default(),
            dict: DictOxide::new(flags),
        }
    }

    /// Get the adler32 checksum of the currently encoded data.
    pub const fn adler32(&self) -> u32 {
        self.params.adler32
    }

    /// Get the return status of the previous [`compress`](fn.compress.html)
    /// call with this compressor.
    pub const fn prev_return_status(&self) -> TDEFLStatus {
        self.params.prev_return_status
    }

    /// Get the raw compressor flags.
    ///
    /// # Notes
    /// This function may be deprecated or changed in the future to use more rust-style flags.
    pub const fn flags(&self) -> i32 {
        self.params.flags as i32
    }

    /// Returns whether the compressor is wrapping the data in a zlib format or not.
    pub const fn data_format(&self) -> DataFormat {
        if (self.params.flags & TDEFL_WRITE_ZLIB_HEADER) != 0 {
            DataFormat::Zlib
        } else {
            DataFormat::Raw
        }
    }

    /// Reset the state of the compressor, keeping the same parameters.
    ///
    /// This avoids re-allocating data.
    pub fn reset(&mut self) {
        // LZ buf and huffman has no settings or dynamic memory
        // that needs to be saved, so we simply replace them.
        self.lz = LZOxide::new();
        self.params.reset();
        *self.huff = HuffmanOxide::default();
        self.dict.reset();
    }

    /// Set the compression level of the compressor.
    ///
    /// Using this to change level after compression has started is supported.
    /// # Notes
    /// The compression strategy will be reset to the default one when this is called.
    pub fn set_compression_level(&mut self, level: CompressionLevel) {
        let format = self.data_format();
        self.set_format_and_level(format, level as u8);
    }

    /// Set the compression level of the compressor using an integer value.
    ///
    /// Using this to change level after compression has started is supported.
    /// # Notes
    /// The compression strategy will be reset to the default one when this is called.
    pub fn set_compression_level_raw(&mut self, level: u8) {
        let format = self.data_format();
        self.set_format_and_level(format, level);
    }

    /// Update the compression settings of the compressor.
    ///
    /// Changing the `DataFormat` after compression has started will result in
    /// a corrupted stream.
    ///
    /// # Notes
    /// This function mainly intended for setting the initial settings after e.g creating with
    /// `default` or after calling `CompressorOxide::reset()`, and behaviour may be changed
    /// to disallow calling it after starting compression in the future.
    pub fn set_format_and_level(&mut self, data_format: DataFormat, level: u8) {
        let flags = create_comp_flags_from_zip_params(
            level.into(),
            data_format.to_window_bits(),
            CompressionStrategy::Default as i32,
        );
        self.params.update_flags(flags);
        self.dict.update_flags(flags);
    }
}

impl Default for CompressorOxide {
    /// Initialize the compressor with a level of 4, zlib wrapper and
    /// the default strategy.
    fn default() -> Self {
        CompressorOxide {
            lz: LZOxide::new(),
            params: ParamsOxide::new(DEFAULT_FLAGS),
            huff: Box::default(),
            dict: DictOxide::new(DEFAULT_FLAGS),
        }
    }
}

/// Callback function and user used in `compress_to_output`.
pub struct CallbackFunc<'a> {
    pub put_buf_func: &'a mut dyn FnMut(&[u8]) -> bool,
}

impl CallbackFunc<'_> {
    fn flush_output(
        &mut self,
        saved_output: SavedOutputBufferOxide,
        params: &mut ParamsOxide,
    ) -> i32 {
        // TODO: As this could be unsafe since
        // we can't verify the function pointer
        // this whole function should maybe be unsafe as well.
        let call_success = (self.put_buf_func)(&params.local_buf.b[0..saved_output.pos]);

        if !call_success {
            params.prev_return_status = TDEFLStatus::PutBufFailed;
            return params.prev_return_status as i32;
        }

        params.flush_remaining as i32
    }
}

struct CallbackBuf<'a> {
    pub out_buf: &'a mut [u8],
}

impl CallbackBuf<'_> {
    fn flush_output(
        &mut self,
        saved_output: SavedOutputBufferOxide,
        params: &mut ParamsOxide,
    ) -> i32 {
        if saved_output.local {
            let n = cmp::min(saved_output.pos, self.out_buf.len() - params.out_buf_ofs);
            (self.out_buf[params.out_buf_ofs..params.out_buf_ofs + n])
                .copy_from_slice(&params.local_buf.b[..n]);

            params.out_buf_ofs += n;
            if saved_output.pos != n {
                params.flush_ofs = n as u32;
                params.flush_remaining = (saved_output.pos - n) as u32;
            }
        } else {
            params.out_buf_ofs += saved_output.pos;
        }

        params.flush_remaining as i32
    }
}

enum CallbackOut<'a> {
    Func(CallbackFunc<'a>),
    Buf(CallbackBuf<'a>),
}

impl CallbackOut<'_> {
    fn new_output_buffer<'b>(
        &'b mut self,
        local_buf: &'b mut [u8],
        out_buf_ofs: usize,
    ) -> OutputBufferOxide<'b> {
        let is_local;
        let buf_len = OUT_BUF_SIZE - 16;
        let chosen_buffer = match *self {
            CallbackOut::Buf(ref mut cb) if cb.out_buf.len() - out_buf_ofs >= OUT_BUF_SIZE => {
                is_local = false;
                &mut cb.out_buf[out_buf_ofs..out_buf_ofs + buf_len]
            }
            _ => {
                is_local = true;
                &mut local_buf[..buf_len]
            }
        };

        OutputBufferOxide {
            inner: chosen_buffer,
            inner_pos: 0,
            local: is_local,
            bit_buffer: 0,
            bits_in: 0,
        }
    }
}

pub(crate) struct CallbackOxide<'a> {
    in_buf: Option<&'a [u8]>,
    in_buf_size: Option<&'a mut usize>,
    out_buf_size: Option<&'a mut usize>,
    out: CallbackOut<'a>,
}

impl<'a> CallbackOxide<'a> {
    fn new_callback_buf(in_buf: &'a [u8], out_buf: &'a mut [u8]) -> Self {
        CallbackOxide {
            in_buf: Some(in_buf),
            in_buf_size: None,
            out_buf_size: None,
            out: CallbackOut::Buf(CallbackBuf { out_buf }),
        }
    }

    fn new_callback_func(in_buf: &'a [u8], callback_func: CallbackFunc<'a>) -> Self {
        CallbackOxide {
            in_buf: Some(in_buf),
            in_buf_size: None,
            out_buf_size: None,
            out: CallbackOut::Func(callback_func),
        }
    }

    fn update_size(&mut self, in_size: Option<usize>, out_size: Option<usize>) {
        if let (Some(in_size), Some(size)) = (in_size, self.in_buf_size.as_mut()) {
            **size = in_size;
        }

        if let (Some(out_size), Some(size)) = (out_size, self.out_buf_size.as_mut()) {
            **size = out_size
        }
    }

    fn flush_output(
        &mut self,
        saved_output: SavedOutputBufferOxide,
        params: &mut ParamsOxide,
    ) -> i32 {
        if saved_output.pos == 0 {
            return params.flush_remaining as i32;
        }

        self.update_size(Some(params.src_pos), None);
        match self.out {
            CallbackOut::Func(ref mut cf) => cf.flush_output(saved_output, params),
            CallbackOut::Buf(ref mut cb) => cb.flush_output(saved_output, params),
        }
    }

    pub(crate) fn buf(&mut self) -> Option<&'a [u8]> {
        self.in_buf
    }
}

struct OutputBufferOxide<'a> {
    pub inner: &'a mut [u8],
    pub inner_pos: usize,
    pub local: bool,

    pub bit_buffer: u32,
    pub bits_in: u32,
}

impl OutputBufferOxide<'_> {
    /// Write bits to the bit buffer and flushes
    /// the bit buffer so any whole bytes are output
    /// to the underlying buffer.
    fn put_bits(&mut self, bits: u32, len: u32) {
        // TODO: Removing this assertion worsens performance
        // Need to figure out why
        assert!(bits <= ((1u32 << len) - 1u32));
        self.bit_buffer |= bits << self.bits_in;
        self.bits_in += len;

        while self.bits_in >= 8 {
            self.inner[self.inner_pos] = self.bit_buffer as u8;
            self.inner_pos += 1;
            self.bit_buffer >>= 8;
            self.bits_in -= 8;
        }
    }

    #[inline]
    /// Write the provided bits to the bit buffer without flushing
    /// anything. Does not check if there is actually space for it.
    fn put_bits_no_flush(&mut self, bits: u32, len: u32) {
        self.bit_buffer |= bits << self.bits_in;
        self.bits_in += len;
    }

    const fn save(&self) -> SavedOutputBufferOxide {
        SavedOutputBufferOxide {
            pos: self.inner_pos,
            bit_buffer: self.bit_buffer,
            bits_in: self.bits_in,
            local: self.local,
        }
    }

    fn load(&mut self, saved: SavedOutputBufferOxide) {
        self.inner_pos = saved.pos;
        self.bit_buffer = saved.bit_buffer;
        self.bits_in = saved.bits_in;
        self.local = saved.local;
    }

    #[inline]
    /// Pad the bit buffer to a whole byte with
    /// zeroes and write that byte to the output buffer.
    fn pad_to_bytes(&mut self) {
        if self.bits_in != 0 {
            let len = 8 - self.bits_in;
            self.put_bits(0, len);
        }
    }

    #[inline]
    fn write_bytes(&mut self, bytes: &[u8]) {
        debug_assert_eq!(self.bits_in, 0);
        self.inner[self.inner_pos..self.inner_pos + bytes.len()].copy_from_slice(bytes);
        self.inner_pos += bytes.len();
    }
}

struct SavedOutputBufferOxide {
    pub pos: usize,
    pub bit_buffer: u32,
    pub bits_in: u32,
    pub local: bool,
}

struct BitBuffer {
    pub bit_buffer: u64,
    pub bits_in: u32,
}

impl BitBuffer {
    fn put_fast(&mut self, bits: u64, len: u32) {
        self.bit_buffer |= bits << self.bits_in;
        self.bits_in += len;
    }

    fn flush(&mut self, output: &mut OutputBufferOxide) -> Result<()> {
        let pos = output.inner_pos;
        {
            // isolation to please borrow checker
            let inner = &mut output.inner[pos..pos + 8];
            let bytes = u64::to_le_bytes(self.bit_buffer);
            inner.copy_from_slice(&bytes);
        }
        match output.inner_pos.checked_add((self.bits_in >> 3) as usize) {
            Some(n) if n <= output.inner.len() => output.inner_pos = n,
            _ => return Err(Error {}),
        }
        self.bit_buffer >>= self.bits_in & !7;
        self.bits_in &= 7;
        Ok(())
    }
}

/// A struct containing data about huffman codes and symbol frequencies.
///
/// NOTE: Only the literal/lengths have enough symbols to actually use
/// the full array. It's unclear why it's defined like this in miniz,
/// it could be for cache/alignment reasons.
pub(crate) struct HuffmanOxide {
    /// Number of occurrences of each symbol.
    pub count: [[u16; MAX_HUFF_SYMBOLS]; MAX_HUFF_TABLES],
    /// The bits of the huffman code assigned to the symbol
    pub codes: [[u16; MAX_HUFF_SYMBOLS]; MAX_HUFF_TABLES],
    /// The length of the huffman code assigned to the symbol.
    pub code_sizes: [[u8; MAX_HUFF_SYMBOLS]; MAX_HUFF_TABLES],
}

/// Tables used for literal/lengths in `HuffmanOxide`.
const LITLEN_TABLE: usize = 0;
/// Tables for distances.
const DIST_TABLE: usize = 1;
/// Tables for the run-length encoded huffman lengths for literals/lengths/distances.
const HUFF_CODES_TABLE: usize = 2;

/// Status of RLE encoding of huffman code lengths.
struct Rle {
    pub z_count: u32,
    pub repeat_count: u16,
    pub prev_code_size: u8,
}

impl Rle {
    fn prev_code_size(
        &mut self,
        packed_code_sizes: &mut [u8],
        packed_pos: &mut usize,
        h: &mut HuffmanOxide,
    ) -> Result<()> {
        let mut write = |buf| write(buf, packed_code_sizes, packed_pos);
        let counts = &mut h.count[HUFF_CODES_TABLE];
        if self.repeat_count != 0 {
            if self.repeat_count < 3 {
                counts[self.prev_code_size as usize] =
                    counts[self.prev_code_size as usize].wrapping_add(self.repeat_count);
                let code = self.prev_code_size;
                write(&[code, code, code][..self.repeat_count as usize])?;
            } else {
                counts[16] = counts[16].wrapping_add(1);
                write(&[16, (self.repeat_count - 3) as u8][..])?;
            }
            self.repeat_count = 0;
        }

        Ok(())
    }

    fn zero_code_size(
        &mut self,
        packed_code_sizes: &mut [u8],
        packed_pos: &mut usize,
        h: &mut HuffmanOxide,
    ) -> Result<()> {
        let mut write = |buf| write(buf, packed_code_sizes, packed_pos);
        let counts = &mut h.count[HUFF_CODES_TABLE];
        if self.z_count != 0 {
            if self.z_count < 3 {
                counts[0] = counts[0].wrapping_add(self.z_count as u16);
                write(&[0, 0, 0][..self.z_count as usize])?;
            } else if self.z_count <= 10 {
                counts[17] = counts[17].wrapping_add(1);
                write(&[17, (self.z_count - 3) as u8][..])?;
            } else {
                counts[18] = counts[18].wrapping_add(1);
                write(&[18, (self.z_count - 11) as u8][..])?;
            }
            self.z_count = 0;
        }

        Ok(())
    }
}

fn write(src: &[u8], dst: &mut [u8], dst_pos: &mut usize) -> Result<()> {
    match dst.get_mut(*dst_pos..*dst_pos + src.len()) {
        Some(s) => s.copy_from_slice(src),
        None => return Err(Error {}),
    }
    *dst_pos += src.len();
    Ok(())
}

impl Default for HuffmanOxide {
    fn default() -> Self {
        HuffmanOxide {
            count: [[0; MAX_HUFF_SYMBOLS]; MAX_HUFF_TABLES],
            codes: [[0; MAX_HUFF_SYMBOLS]; MAX_HUFF_TABLES],
            code_sizes: [[0; MAX_HUFF_SYMBOLS]; MAX_HUFF_TABLES],
        }
    }
}

impl HuffmanOxide {
    fn radix_sort_symbols<'a>(
        symbols0: &'a mut [SymFreq],
        symbols1: &'a mut [SymFreq],
    ) -> &'a mut [SymFreq] {
        let mut hist = [[0; 256]; 2];

        for freq in symbols0.iter() {
            hist[0][(freq.key & 0xFF) as usize] += 1;
            hist[1][((freq.key >> 8) & 0xFF) as usize] += 1;
        }

        let mut n_passes = 2;
        if symbols0.len() == hist[1][0] {
            n_passes -= 1;
        }

        let mut current_symbols = symbols0;
        let mut new_symbols = symbols1;

        for (pass, hist_item) in hist.iter().enumerate().take(n_passes) {
            let mut offsets = [0; 256];
            let mut offset = 0;
            for i in 0..256 {
                offsets[i] = offset;
                offset += hist_item[i];
            }

            for sym in current_symbols.iter() {
                let j = ((sym.key >> (pass * 8)) & 0xFF) as usize;
                new_symbols[offsets[j]] = *sym;
                offsets[j] += 1;
            }

            mem::swap(&mut current_symbols, &mut new_symbols);
        }

        current_symbols
    }

    fn calculate_minimum_redundancy(symbols: &mut [SymFreq]) {
        match symbols.len() {
            0 => (),
            1 => symbols[0].key = 1,
            n => {
                symbols[0].key += symbols[1].key;
                let mut root = 0;
                let mut leaf = 2;
                for next in 1..n - 1 {
                    if (leaf >= n) || (symbols[root].key < symbols[leaf].key) {
                        symbols[next].key = symbols[root].key;
                        symbols[root].key = next as u16;
                        root += 1;
                    } else {
                        symbols[next].key = symbols[leaf].key;
                        leaf += 1;
                    }

                    if (leaf >= n) || (root < next && symbols[root].key < symbols[leaf].key) {
                        symbols[next].key = symbols[next].key.wrapping_add(symbols[root].key);
                        symbols[root].key = next as u16;
                        root += 1;
                    } else {
                        symbols[next].key = symbols[next].key.wrapping_add(symbols[leaf].key);
                        leaf += 1;
                    }
                }

                symbols[n - 2].key = 0;
                for next in (0..n - 2).rev() {
                    symbols[next].key = symbols[symbols[next].key as usize].key + 1;
                }

                let mut avbl = 1;
                let mut used = 0;
                let mut dpth = 0;
                let mut root = (n - 2) as i32;
                let mut next = (n - 1) as i32;
                while avbl > 0 {
                    while (root >= 0) && (symbols[root as usize].key == dpth) {
                        used += 1;
                        root -= 1;
                    }
                    while avbl > used {
                        symbols[next as usize].key = dpth;
                        next -= 1;
                        avbl -= 1;
                    }
                    avbl = 2 * used;
                    dpth += 1;
                    used = 0;
                }
            }
        }
    }

    fn enforce_max_code_size(num_codes: &mut [i32], code_list_len: usize, max_code_size: usize) {
        if code_list_len <= 1 {
            return;
        }

        num_codes[max_code_size] += num_codes[max_code_size + 1..].iter().sum::<i32>();
        let total = num_codes[1..=max_code_size]
            .iter()
            .rev()
            .enumerate()
            .fold(0u32, |total, (i, &x)| total + ((x as u32) << i));

        for _ in (1 << max_code_size)..total {
            num_codes[max_code_size] -= 1;
            for i in (1..max_code_size).rev() {
                if num_codes[i] != 0 {
                    num_codes[i] -= 1;
                    num_codes[i + 1] += 2;
                    break;
                }
            }
        }
    }

    fn optimize_table(
        &mut self,
        table_num: usize,
        table_len: usize,
        code_size_limit: usize,
        static_table: bool,
    ) {
        let mut num_codes = [0i32; 32 + 1];
        let mut next_code = [0u32; MAX_SUPPORTED_HUFF_CODESIZE + 1];

        if static_table {
            for &code_size in &self.code_sizes[table_num][..table_len] {
                num_codes[code_size as usize] += 1;
            }
        } else {
            let mut symbols0 = [SymFreq {
                key: 0,
                sym_index: 0,
            }; MAX_HUFF_SYMBOLS];
            let mut symbols1 = [SymFreq {
                key: 0,
                sym_index: 0,
            }; MAX_HUFF_SYMBOLS];

            let mut num_used_symbols = 0;
            for i in 0..table_len {
                if self.count[table_num][i] != 0 {
                    symbols0[num_used_symbols] = SymFreq {
                        key: self.count[table_num][i],
                        sym_index: i as u16,
                    };
                    num_used_symbols += 1;
                }
            }

            let symbols = Self::radix_sort_symbols(
                &mut symbols0[..num_used_symbols],
                &mut symbols1[..num_used_symbols],
            );
            Self::calculate_minimum_redundancy(symbols);

            for symbol in symbols.iter() {
                num_codes[symbol.key as usize] += 1;
            }

            Self::enforce_max_code_size(&mut num_codes, num_used_symbols, code_size_limit);

            self.code_sizes[table_num].fill(0);
            self.codes[table_num].fill(0);

            let mut last = num_used_symbols;
            for (i, &num_item) in num_codes
                .iter()
                .enumerate()
                .take(code_size_limit + 1)
                .skip(1)
            {
                let first = last - num_item as usize;
                for symbol in &symbols[first..last] {
                    self.code_sizes[table_num][symbol.sym_index as usize] = i as u8;
                }
                last = first;
            }
        }

        let mut j = 0;
        next_code[1] = 0;
        for i in 2..=code_size_limit {
            j = (j + num_codes[i - 1]) << 1;
            next_code[i] = j as u32;
        }

        for (&code_size, huff_code) in self.code_sizes[table_num]
            .iter()
            .take(table_len)
            .zip(self.codes[table_num].iter_mut().take(table_len))
        {
            if code_size == 0 {
                continue;
            }

            let code = next_code[code_size as usize];

            next_code[code_size as usize] += 1;

            let rev_code = (code as u16).reverse_bits() >> (16 - code_size);

            *huff_code = rev_code;
        }
    }

    fn start_static_block(&mut self, output: &mut OutputBufferOxide) {
        self.code_sizes[LITLEN_TABLE][0..144].fill(8);
        self.code_sizes[LITLEN_TABLE][144..256].fill(9);
        self.code_sizes[LITLEN_TABLE][256..280].fill(7);
        self.code_sizes[LITLEN_TABLE][280..288].fill(8);

        self.code_sizes[DIST_TABLE][..32].fill(5);

        self.optimize_table(LITLEN_TABLE, 288, 15, true);
        self.optimize_table(DIST_TABLE, 32, 15, true);

        output.put_bits(0b01, 2)
    }

    fn start_dynamic_block(&mut self, output: &mut OutputBufferOxide) -> Result<()> {
        // There will always be one, and only one end of block code.
        self.count[0][256] = 1;

        self.optimize_table(0, MAX_HUFF_SYMBOLS_0, 15, false);
        self.optimize_table(1, MAX_HUFF_SYMBOLS_1, 15, false);

        let num_lit_codes = 286
            - &self.code_sizes[0][257..286]
                .iter()
                .rev()
                .take_while(|&x| *x == 0)
                .count();

        let num_dist_codes = 30
            - &self.code_sizes[1][1..30]
                .iter()
                .rev()
                .take_while(|&x| *x == 0)
                .count();

        let mut code_sizes_to_pack = [0u8; MAX_HUFF_SYMBOLS_0 + MAX_HUFF_SYMBOLS_1];
        let mut packed_code_sizes = [0u8; MAX_HUFF_SYMBOLS_0 + MAX_HUFF_SYMBOLS_1];

        let total_code_sizes_to_pack = num_lit_codes + num_dist_codes;

        code_sizes_to_pack[..num_lit_codes].copy_from_slice(&self.code_sizes[0][..num_lit_codes]);

        code_sizes_to_pack[num_lit_codes..total_code_sizes_to_pack]
            .copy_from_slice(&self.code_sizes[1][..num_dist_codes]);

        let mut rle = Rle {
            z_count: 0,
            repeat_count: 0,
            prev_code_size: 0xFF,
        };

        self.count[HUFF_CODES_TABLE][..MAX_HUFF_SYMBOLS_2].fill(0);

        let mut packed_pos = 0;
        for &code_size in &code_sizes_to_pack[..total_code_sizes_to_pack] {
            if code_size == 0 {
                rle.prev_code_size(&mut packed_code_sizes, &mut packed_pos, self)?;
                rle.z_count += 1;
                if rle.z_count == 138 {
                    rle.zero_code_size(&mut packed_code_sizes, &mut packed_pos, self)?;
                }
            } else {
                rle.zero_code_size(&mut packed_code_sizes, &mut packed_pos, self)?;
                if code_size != rle.prev_code_size {
                    rle.prev_code_size(&mut packed_code_sizes, &mut packed_pos, self)?;
                    self.count[HUFF_CODES_TABLE][code_size as usize] =
                        self.count[HUFF_CODES_TABLE][code_size as usize].wrapping_add(1);
                    write(&[code_size], &mut packed_code_sizes, &mut packed_pos)?;
                } else {
                    rle.repeat_count += 1;
                    if rle.repeat_count == 6 {
                        rle.prev_code_size(&mut packed_code_sizes, &mut packed_pos, self)?;
                    }
                }
            }
            rle.prev_code_size = code_size;
        }

        if rle.repeat_count != 0 {
            rle.prev_code_size(&mut packed_code_sizes, &mut packed_pos, self)?;
        } else {
            rle.zero_code_size(&mut packed_code_sizes, &mut packed_pos, self)?;
        }

        self.optimize_table(2, MAX_HUFF_SYMBOLS_2, 7, false);

        output.put_bits_no_flush(2, 2);

        output.put_bits_no_flush((num_lit_codes - 257) as u32, 5);
        output.put_bits_no_flush((num_dist_codes - 1) as u32, 5);

        let mut num_bit_lengths = 18
            - HUFFMAN_LENGTH_ORDER
                .iter()
                .rev()
                .take_while(|&swizzle| self.code_sizes[HUFF_CODES_TABLE][*swizzle as usize] == 0)
                .count();

        num_bit_lengths = cmp::max(4, num_bit_lengths + 1);
        output.put_bits(num_bit_lengths as u32 - 4, 4);
        for &swizzle in &HUFFMAN_LENGTH_ORDER[..num_bit_lengths] {
            output.put_bits(
                u32::from(self.code_sizes[HUFF_CODES_TABLE][swizzle as usize]),
                3,
            );
        }

        let mut packed_code_size_index = 0;
        while packed_code_size_index < packed_pos {
            let code = packed_code_sizes[packed_code_size_index] as usize;
            packed_code_size_index += 1;
            assert!(code < MAX_HUFF_SYMBOLS_2);
            output.put_bits(
                u32::from(self.codes[HUFF_CODES_TABLE][code]),
                u32::from(self.code_sizes[HUFF_CODES_TABLE][code]),
            );
            if code >= 16 {
                output.put_bits(
                    u32::from(packed_code_sizes[packed_code_size_index]),
                    [2, 3, 7][code - 16],
                );
                packed_code_size_index += 1;
            }
        }

        Ok(())
    }
}

pub(crate) struct DictOxide {
    /// The maximum number of checks in the hash chain, for the initial,
    /// and the lazy match respectively.
    pub max_probes: [u32; 2],
    /// Buffer of input data.
    /// Padded with 1 byte to simplify matching code in `compress_fast`.
    pub b: HashBuffers,

    pub code_buf_dict_pos: usize,
    pub lookahead_size: usize,
    pub lookahead_pos: usize,
    pub size: usize,
    loop_len: u8,
}

const fn probes_from_flags(flags: u32) -> [u32; 2] {
    [
        1 + ((flags & 0xFFF) + 2) / 3,
        1 + (((flags & 0xFFF) >> 2) + 2) / 3,
    ]
}

impl DictOxide {
    fn new(flags: u32) -> Self {
        DictOxide {
            max_probes: probes_from_flags(flags),
            b: HashBuffers::default(),
            code_buf_dict_pos: 0,
            lookahead_size: 0,
            lookahead_pos: 0,
            size: 0,
            loop_len: 32,
        }
    }

    fn update_flags(&mut self, flags: u32) {
        self.max_probes = probes_from_flags(flags);
    }

    fn reset(&mut self) {
        self.b.reset();
        self.code_buf_dict_pos = 0;
        self.lookahead_size = 0;
        self.lookahead_pos = 0;
        self.size = 0;
    }

    /// Do an unaligned read of the data at `pos` in the dictionary and treat it as if it was of
    /// type T.
    #[inline]
    fn read_unaligned_u32(&self, pos: usize) -> u32 {
        // Masking the value here helps avoid bounds checks.
        let pos = pos & LZ_DICT_SIZE_MASK;
        let end = pos + 4;
        // Somehow this assertion makes things faster.
        // TODO: as of may 2024 this does not seem to make any difference
        // so consider removing.
        assert!(end < LZ_DICT_FULL_SIZE);

        let bytes: [u8; 4] = self.b.dict[pos..end].try_into().unwrap();
        u32::from_le_bytes(bytes)
    }

    /// Do an unaligned read of the data at `pos` in the dictionary and treat it as if it was of
    /// type T.
    #[inline]
    fn read_unaligned_u64(&self, pos: usize) -> u64 {
        // Help evade bounds/panic code check by masking the position value
        // This provides a small speedup at the cost of an instruction or two instead of
        // having to use unsafe.
        let pos = pos & LZ_DICT_SIZE_MASK;
        let bytes: [u8; 8] = self.b.dict[pos..pos + 8].try_into().unwrap();
        u64::from_le_bytes(bytes)
    }

    /// Try to find a match for the data at lookahead_pos in the dictionary that is
    /// longer than `match_len`.
    /// Returns a tuple containing (match_distance, match_length). Will be equal to the input
    /// values if no better matches were found.
    fn find_match(
        &self,
        lookahead_pos: usize,
        max_dist: usize,
        max_match_len: u32,
        mut match_dist: u32,
        mut match_len: u32,
    ) -> (u32, u32) {
        // Clamp the match len and max_match_len to be valid. (It should be when this is called, but
        // do it for now just in case for safety reasons.)
        // This should normally end up as at worst conditional moves,
        // so it shouldn't slow us down much.
        // TODO: Statically verify these so we don't need to do this.
        let max_match_len = cmp::min(MAX_MATCH_LEN as u32, max_match_len);
        match_len = cmp::max(match_len, 1);

        // If we already have a match of the full length don't bother searching for another one.
        if max_match_len <= match_len {
            return (match_dist, match_len);
        }

        let pos = lookahead_pos & LZ_DICT_SIZE_MASK;
        let mut probe_pos = pos;
        // Number of probes into the hash chains.
        let mut num_probes_left = if match_len < 32 {
            self.max_probes[0]
        } else {
            self.max_probes[1]
        };

        // Read the last byte of the current match, and the next one, used to compare matches.
        let mut c01: u16 = read_u16_le(&self.b.dict, pos + match_len as usize - 1);
        // Read the two bytes at the end position of the current match.
        let s01: u16 = read_u16_le(&self.b.dict, pos);

        'outer: loop {
            let mut dist;
            'found: loop {
                num_probes_left -= 1;
                if num_probes_left == 0 {
                    // We have done as many probes in the hash chain as the current compression
                    // settings allow, so return the best match we found, if any.
                    return (match_dist, match_len);
                }

                for _ in 0..3 {
                    let next_probe_pos = self.b.next[probe_pos] as usize;

                    dist = (lookahead_pos - next_probe_pos) & 0xFFFF;
                    // Optimization: The last condition should never be hit but helps the compiler by avoiding
                    // doing the bounds check in the read_u16_le call and adding the extra instructions
                    // for branching to a panic after that and instead just adds the extra instruction here
                    // instead saving some instructions and thus improving performance a bit.
                    // May want to investigate whether we can avoid it entirely but as of now the compiler
                    // isn't able to deduce that match_len - 1 is bounded to [1-257]
                    // Disable clippy lint as it needs to be written in this specific way
                    // rather than MAX_MATCH_LEN to work
                    // because the compiler isn't super smart....
                    #[allow(clippy::int_plus_one)]
                    if next_probe_pos == 0
                        || dist > max_dist
                        || match_len as usize - 1 >= MAX_MATCH_LEN
                    {
                        // We reached the end of the hash chain, or the next value is further away
                        // than the maximum allowed distance, so return the best match we found, if
                        // any.
                        return (match_dist, match_len);
                    }

                    // Mask the position value to get the position in the hash chain of the next
                    // position to match against.
                    probe_pos = next_probe_pos & LZ_DICT_SIZE_MASK;

                    if read_u16_le(&self.b.dict, probe_pos + match_len as usize - 1) == c01 {
                        break 'found;
                    }
                }
            }

            if dist == 0 {
                // We've looked through the whole match range, so return the best match we
                // found.
                return (match_dist, match_len);
            }

            // Check if the two first bytes match.
            if read_u16_le(&self.b.dict, probe_pos) != s01 {
                continue;
            }

            let mut p = pos + 2;
            let mut q = probe_pos + 2;
            // The first two bytes matched, so check the full length of the match.
            // TODO: This is a workaround for an upstream issue introduced after a LLVM upgrade in rust 1.82.
            // the compiler is too smart and ends up unrolling the loop which causes the performance to get worse
            // Using a variable instead of a constant here to prevent it seems to at least get back some of the performance loss.
            for _ in 0..self.loop_len as i32 {
                let p_data: u64 = self.read_unaligned_u64(p);
                let q_data: u64 = self.read_unaligned_u64(q);
                // Compare of 8 bytes at a time by using unaligned loads of 64-bit integers.
                let xor_data = p_data ^ q_data;
                if xor_data == 0 {
                    p += 8;
                    q += 8;
                } else {
                    // If not all of the last 8 bytes matched, check how may of them did.
                    let trailing = xor_data.trailing_zeros();

                    let probe_len = p - pos + (trailing as usize >> 3);
                    if probe_len > match_len as usize {
                        match_dist = dist as u32;
                        match_len = cmp::min(max_match_len, probe_len as u32);
                        if match_len >= max_match_len {
                            // We found a match that had the maximum allowed length,
                            // so there is now point searching further.
                            return (match_dist, match_len);
                        }
                        // We found a better match, so save the last two bytes for further match
                        // comparisons.
                        // Optimization: use saturating_sub makes the compiler able to evade the bounds check
                        // at the cost of some extra instructions since it avoids any possibility of wraparound.
                        // need to see if we can find a better way to do this since this is still a bit costly.
                        c01 =
                            read_u16_le(&self.b.dict, (pos + match_len as usize).saturating_sub(1));
                    }
                    continue 'outer;
                }
            }

            return (dist as u32, cmp::min(max_match_len, MAX_MATCH_LEN as u32));
        }
    }
}

pub(crate) struct ParamsOxide {
    pub flags: u32,
    pub greedy_parsing: bool,
    pub block_index: u32,

    pub saved_match_dist: u32,
    pub saved_match_len: u32,
    pub saved_lit: u8,

    pub flush: TDEFLFlush,
    pub flush_ofs: u32,
    pub flush_remaining: u32,
    pub finished: bool,

    pub adler32: u32,

    pub src_pos: usize,

    pub out_buf_ofs: usize,
    pub prev_return_status: TDEFLStatus,

    pub saved_bit_buffer: u32,
    pub saved_bits_in: u32,

    pub local_buf: Box<LocalBuf>,
}

impl ParamsOxide {
    fn new(flags: u32) -> Self {
        ParamsOxide {
            flags,
            greedy_parsing: flags & TDEFL_GREEDY_PARSING_FLAG != 0,
            block_index: 0,
            saved_match_dist: 0,
            saved_match_len: 0,
            saved_lit: 0,
            flush: TDEFLFlush::None,
            flush_ofs: 0,
            flush_remaining: 0,
            finished: false,
            adler32: MZ_ADLER32_INIT,
            src_pos: 0,
            out_buf_ofs: 0,
            prev_return_status: TDEFLStatus::Okay,
            saved_bit_buffer: 0,
            saved_bits_in: 0,
            local_buf: Box::default(),
        }
    }

    fn update_flags(&mut self, flags: u32) {
        self.flags = flags;
        self.greedy_parsing = self.flags & TDEFL_GREEDY_PARSING_FLAG != 0;
    }

    /// Reset state, saving settings.
    fn reset(&mut self) {
        self.block_index = 0;
        self.saved_match_len = 0;
        self.saved_match_dist = 0;
        self.saved_lit = 0;
        self.flush = TDEFLFlush::None;
        self.flush_ofs = 0;
        self.flush_remaining = 0;
        self.finished = false;
        self.adler32 = MZ_ADLER32_INIT;
        self.src_pos = 0;
        self.out_buf_ofs = 0;
        self.prev_return_status = TDEFLStatus::Okay;
        self.saved_bit_buffer = 0;
        self.saved_bits_in = 0;
        self.local_buf.b = [0; OUT_BUF_SIZE];
    }
}

pub(crate) struct LZOxide {
    pub codes: [u8; LZ_CODE_BUF_SIZE],
    pub code_position: usize,
    pub flag_position: usize,

    // The total number of bytes in the current block.
    pub total_bytes: u32,
    pub num_flags_left: u32,
}

impl LZOxide {
    const fn new() -> Self {
        LZOxide {
            codes: [0; LZ_CODE_BUF_SIZE],
            code_position: 1,
            flag_position: 0,
            total_bytes: 0,
            num_flags_left: 8,
        }
    }

    fn write_code(&mut self, val: u8) {
        // Perf - go via u16 to help evade bounds check
        // TODO: see if we can use u16 for flag_position in general.
        self.codes[usize::from(self.code_position as u16)] = val;
        self.code_position += 1;
    }

    fn init_flag(&mut self) {
        if self.num_flags_left == 8 {
            *self.get_flag() = 0;
            self.code_position -= 1;
        } else {
            *self.get_flag() >>= self.num_flags_left;
        }
    }

    fn get_flag(&mut self) -> &mut u8 {
        // Perf - go via u16 to help evade bounds check
        // TODO: see if we can use u16 for flag_position in general.
        &mut self.codes[usize::from(self.flag_position as u16)]
    }

    fn plant_flag(&mut self) {
        self.flag_position = self.code_position;
        self.code_position += 1;
    }

    fn consume_flag(&mut self) {
        self.num_flags_left -= 1;
        if self.num_flags_left == 0 {
            self.num_flags_left = 8;
            self.plant_flag();
        }
    }
}

fn compress_lz_codes(
    huff: &HuffmanOxide,
    output: &mut OutputBufferOxide,
    lz_code_buf: &[u8; LZ_CODE_BUF_SIZE],
    lz_code_buf_used_len: usize,
) -> Result<bool> {
    let mut flags = 1;
    let mut bb = BitBuffer {
        bit_buffer: u64::from(output.bit_buffer),
        bits_in: output.bits_in,
    };

    // Help out the compiler know this variable won't be larger than
    // the buffer length since the constants won't propagate through the function call.
    let lz_code_buf_used_len = cmp::min(lz_code_buf.len(), lz_code_buf_used_len);

    let mut i: usize = 0;
    while i < lz_code_buf_used_len {
        if flags == 1 {
            flags = u32::from(lz_code_buf[i]) | 0x100;
            i += 1;
        }

        // The lz code was a length code
        if flags & 1 == 1 {
            flags >>= 1;

            let sym;
            let num_extra_bits;

            let match_len = lz_code_buf[i & LZ_CODE_BUF_MASK] as usize;

            let match_dist = lz_code_buf[(i + 1) & LZ_CODE_BUF_MASK] as u16
                | ((lz_code_buf[(i + 2) & LZ_CODE_BUF_MASK] as u16) << 8);

            i += 3;

            debug_assert!(huff.code_sizes[0][LEN_SYM[match_len] as usize + LEN_SYM_OFFSET] != 0);
            let len_sym = (LEN_SYM[match_len] & 31) as usize + LEN_SYM_OFFSET;

            bb.put_fast(
                u64::from(huff.codes[0][len_sym]),
                u32::from(huff.code_sizes[0][len_sym]),
            );
            bb.put_fast(
                match_len as u64 & u64::from(BITMASKS[(LEN_EXTRA[match_len] & 7) as usize]),
                u32::from(LEN_EXTRA[match_len]),
            );

            if match_dist < 512 {
                sym = SMALL_DIST_SYM[match_dist as usize] as usize;
                num_extra_bits = SMALL_DIST_EXTRA[match_dist as usize] as usize;
            } else {
                sym = LARGE_DIST_SYM[(match_dist >> 8) as usize] as usize;
                num_extra_bits = LARGE_DIST_EXTRA[(match_dist >> 8) as usize] as usize;
            }

            debug_assert!(huff.code_sizes[1][sym] != 0);
            bb.put_fast(
                u64::from(huff.codes[1][sym]),
                u32::from(huff.code_sizes[1][sym]),
            );
            bb.put_fast(
                u64::from(match_dist) & u64::from(BITMASKS[num_extra_bits & 15]),
                num_extra_bits as u32,
            );
        } else {
            // The lz code was a literal
            for _ in 0..3 {
                flags >>= 1;
                let lit = lz_code_buf[i & LZ_CODE_BUF_MASK];
                i += 1;

                debug_assert!(huff.code_sizes[0][lit as usize] != 0);
                bb.put_fast(
                    u64::from(huff.codes[0][lit as usize]),
                    u32::from(huff.code_sizes[0][lit as usize]),
                );

                if flags & 1 == 1 || i >= lz_code_buf_used_len {
                    break;
                }
            }
        }

        bb.flush(output)?;
    }

    output.bits_in = 0;
    output.bit_buffer = 0;
    while bb.bits_in != 0 {
        let n = cmp::min(bb.bits_in, 16);
        output.put_bits(bb.bit_buffer as u32 & BITMASKS[n as usize], n);
        bb.bit_buffer >>= n;
        bb.bits_in -= n;
    }

    // Output the end of block symbol.
    output.put_bits(
        u32::from(huff.codes[0][256]),
        u32::from(huff.code_sizes[0][256]),
    );

    Ok(true)
}

fn compress_block(
    huff: &mut HuffmanOxide,
    output: &mut OutputBufferOxide,
    lz: &LZOxide,
    static_block: bool,
) -> Result<bool> {
    if static_block {
        huff.start_static_block(output);
    } else {
        huff.start_dynamic_block(output)?;
    }

    compress_lz_codes(huff, output, &lz.codes, lz.code_position)
}

pub(crate) fn flush_block(
    d: &mut CompressorOxide,
    callback: &mut CallbackOxide,
    flush: TDEFLFlush,
) -> Result<i32> {
    let mut saved_buffer;
    {
        let mut output = callback
            .out
            .new_output_buffer(&mut d.params.local_buf.b, d.params.out_buf_ofs);
        output.bit_buffer = d.params.saved_bit_buffer;
        output.bits_in = d.params.saved_bits_in;

        // TODO: Don't think this second condition should be here but need to verify.
        let use_raw_block = (d.params.flags & TDEFL_FORCE_ALL_RAW_BLOCKS != 0)
            && (d.dict.lookahead_pos - d.dict.code_buf_dict_pos) <= d.dict.size;
        debug_assert_eq!(
            use_raw_block,
            d.params.flags & TDEFL_FORCE_ALL_RAW_BLOCKS != 0
        );

        assert!(d.params.flush_remaining == 0);
        d.params.flush_ofs = 0;
        d.params.flush_remaining = 0;

        d.lz.init_flag();

        // If we are at the start of the stream, write the zlib header if requested.
        if d.params.flags & TDEFL_WRITE_ZLIB_HEADER != 0 && d.params.block_index == 0 {
            let header = zlib::header_from_flags(d.params.flags);
            output.put_bits_no_flush(header[0].into(), 8);
            output.put_bits(header[1].into(), 8);
        }

        // Output the block header.
        output.put_bits((flush == TDEFLFlush::Finish) as u32, 1);

        saved_buffer = output.save();

        let comp_success = if !use_raw_block {
            let use_static =
                (d.params.flags & TDEFL_FORCE_ALL_STATIC_BLOCKS != 0) || (d.lz.total_bytes < 48);
            compress_block(&mut d.huff, &mut output, &d.lz, use_static)?
        } else {
            false
        };

        // If we failed to compress anything and the output would take up more space than the output
        // data, output a stored block instead, which has at most 5 bytes of overhead.
        // We only use some simple heuristics for now.
        // A stored block will have an overhead of at least 4 bytes containing the block length
        // but usually more due to the length parameters having to start at a byte boundary and thus
        // requiring up to 5 bytes of padding.
        // As a static block will have an overhead of at most 1 bit per byte
        // (as literals are either 8 or 9 bytes), a raw block will
        // never take up less space if the number of input bytes are less than 32.
        let expanded = (d.lz.total_bytes > 32)
            && (output.inner_pos - saved_buffer.pos + 1 >= (d.lz.total_bytes as usize))
            && (d.dict.lookahead_pos - d.dict.code_buf_dict_pos <= d.dict.size);

        if use_raw_block || expanded {
            output.load(saved_buffer);

            // Block header.
            output.put_bits(0, 2);

            // Block length has to start on a byte boundary, so pad.
            output.pad_to_bytes();

            // Block length and ones complement of block length.
            output.put_bits(d.lz.total_bytes & 0xFFFF, 16);
            output.put_bits(!d.lz.total_bytes & 0xFFFF, 16);

            // Write the actual bytes.
            let start = d.dict.code_buf_dict_pos & LZ_DICT_SIZE_MASK;
            let end = (d.dict.code_buf_dict_pos + d.lz.total_bytes as usize) & LZ_DICT_SIZE_MASK;
            let dict = &mut d.dict.b.dict;
            if start < end {
                // The data does not wrap around.
                output.write_bytes(&dict[start..end]);
            } else if d.lz.total_bytes > 0 {
                // The data wraps around and the input was not 0 bytes.
                output.write_bytes(&dict[start..LZ_DICT_SIZE]);
                output.write_bytes(&dict[..end]);
            }
        } else if !comp_success {
            output.load(saved_buffer);
            compress_block(&mut d.huff, &mut output, &d.lz, true)?;
        }

        if flush != TDEFLFlush::None {
            if flush == TDEFLFlush::Finish {
                output.pad_to_bytes();
                if d.params.flags & TDEFL_WRITE_ZLIB_HEADER != 0 {
                    let mut adler = d.params.adler32;
                    for _ in 0..4 {
                        output.put_bits((adler >> 24) & 0xFF, 8);
                        adler <<= 8;
                    }
                }
            } else {
                // Sync or Full flush.
                // Output an empty raw block.
                output.put_bits(0, 3);
                output.pad_to_bytes();
                output.put_bits(0, 16);
                output.put_bits(0xFFFF, 16);
            }
        }

        d.huff.count[0][..MAX_HUFF_SYMBOLS_0].fill(0);
        d.huff.count[1][..MAX_HUFF_SYMBOLS_1].fill(0);

        // Clear LZ buffer for the next block.
        d.lz.code_position = 1;
        d.lz.flag_position = 0;
        d.lz.num_flags_left = 8;
        d.dict.code_buf_dict_pos += d.lz.total_bytes as usize;
        d.lz.total_bytes = 0;
        d.params.block_index += 1;

        saved_buffer = output.save();

        d.params.saved_bit_buffer = saved_buffer.bit_buffer;
        d.params.saved_bits_in = saved_buffer.bits_in;
    }

    Ok(callback.flush_output(saved_buffer, &mut d.params))
}

pub(crate) fn record_literal(h: &mut HuffmanOxide, lz: &mut LZOxide, lit: u8) {
    lz.total_bytes += 1;
    lz.write_code(lit);

    *lz.get_flag() >>= 1;
    lz.consume_flag();

    h.count[0][lit as usize] += 1;
}

fn record_match(h: &mut HuffmanOxide, lz: &mut LZOxide, match_len: u32, mut match_dist: u32) {
    debug_assert!(match_len >= MIN_MATCH_LEN.into());
    debug_assert!(match_dist >= 1);
    debug_assert!(match_dist as usize <= LZ_DICT_SIZE);

    lz.total_bytes += match_len;
    match_dist -= 1;
    let match_len = (match_len - u32::from(MIN_MATCH_LEN)) as u8;
    lz.write_code(match_len);
    lz.write_code(match_dist as u8);
    lz.write_code((match_dist >> 8) as u8);

    *lz.get_flag() >>= 1;
    *lz.get_flag() |= 0x80;
    lz.consume_flag();

    let symbol = if match_dist < 512 {
        SMALL_DIST_SYM[match_dist as usize]
    } else {
        LARGE_DIST_SYM[((match_dist >> 8) & 127) as usize]
    } as usize;
    h.count[1][symbol] += 1;
    // Mask the values from LEN_SYM here as the compiler isn't quite smart enough to infer
    // that it only contains values smaller than 32.
    h.count[0][(LEN_SYM[match_len as usize] as usize & 31) + LEN_SYM_OFFSET] += 1;
}

fn compress_normal(d: &mut CompressorOxide, callback: &mut CallbackOxide) -> bool {
    let in_buf = match callback.in_buf {
        None => return true,
        Some(in_buf) => in_buf,
    };

    let mut src_pos = d.params.src_pos;
    let mut lookahead_size = d.dict.lookahead_size;
    let mut lookahead_pos = d.dict.lookahead_pos;
    let mut saved_lit = d.params.saved_lit;
    let mut saved_match_dist = d.params.saved_match_dist;
    let mut saved_match_len = d.params.saved_match_len;

    while src_pos < in_buf.len() || (d.params.flush != TDEFLFlush::None && lookahead_size != 0) {
        let src_buf_left = in_buf.len() - src_pos;
        let num_bytes_to_process = cmp::min(src_buf_left, MAX_MATCH_LEN - lookahead_size);

        if lookahead_size + d.dict.size >= usize::from(MIN_MATCH_LEN) - 1
            && num_bytes_to_process > 0
        {
            let dictb = &mut d.dict.b;

            let mut dst_pos = (lookahead_pos + lookahead_size) & LZ_DICT_SIZE_MASK;
            let mut ins_pos = lookahead_pos + lookahead_size - 2;
            // Start the hash value from the first two bytes
            let mut hash = update_hash(
                u16::from(dictb.dict[ins_pos & LZ_DICT_SIZE_MASK]),
                dictb.dict[(ins_pos + 1) & LZ_DICT_SIZE_MASK],
            );

            lookahead_size += num_bytes_to_process;

            for &c in &in_buf[src_pos..src_pos + num_bytes_to_process] {
                // Add byte to input buffer.
                dictb.dict[dst_pos] = c;
                if dst_pos < MAX_MATCH_LEN - 1 {
                    dictb.dict[LZ_DICT_SIZE + dst_pos] = c;
                }

                // Generate hash from the current byte,
                hash = update_hash(hash, c);
                dictb.next[ins_pos & LZ_DICT_SIZE_MASK] = dictb.hash[hash as usize];
                // and insert it into the hash chain.
                dictb.hash[hash as usize] = ins_pos as u16;
                dst_pos = (dst_pos + 1) & LZ_DICT_SIZE_MASK;
                ins_pos += 1;
            }

            src_pos += num_bytes_to_process;
        } else {
            let dictb = &mut d.dict.b;
            for &c in &in_buf[src_pos..src_pos + num_bytes_to_process] {
                let dst_pos = (lookahead_pos + lookahead_size) & LZ_DICT_SIZE_MASK;
                dictb.dict[dst_pos] = c;
                if dst_pos < MAX_MATCH_LEN - 1 {
                    dictb.dict[LZ_DICT_SIZE + dst_pos] = c;
                }

                lookahead_size += 1;
                if lookahead_size + d.dict.size >= MIN_MATCH_LEN.into() {
                    let ins_pos = lookahead_pos + lookahead_size - 3;
                    let hash = ((u32::from(dictb.dict[ins_pos & LZ_DICT_SIZE_MASK])
                        << (LZ_HASH_SHIFT * 2))
                        ^ ((u32::from(dictb.dict[(ins_pos + 1) & LZ_DICT_SIZE_MASK])
                            << LZ_HASH_SHIFT)
                            ^ u32::from(c)))
                        & (LZ_HASH_SIZE as u32 - 1);

                    dictb.next[ins_pos & LZ_DICT_SIZE_MASK] = dictb.hash[hash as usize];
                    dictb.hash[hash as usize] = ins_pos as u16;
                }
            }

            src_pos += num_bytes_to_process;
        }

        d.dict.size = cmp::min(LZ_DICT_SIZE - lookahead_size, d.dict.size);
        if d.params.flush == TDEFLFlush::None && lookahead_size < MAX_MATCH_LEN {
            break;
        }

        let mut len_to_move = 1;
        let mut cur_match_dist = 0;
        let mut cur_match_len = if saved_match_len != 0 {
            saved_match_len
        } else {
            u32::from(MIN_MATCH_LEN) - 1
        };
        let cur_pos = lookahead_pos & LZ_DICT_SIZE_MASK;
        if d.params.flags & TDEFL_RLE_MATCHES != 0 {
            // If TDEFL_RLE_MATCHES is set, we only look for repeating sequences of the current byte.
            if d.dict.size != 0 {
                let c = d.dict.b.dict[(cur_pos.wrapping_sub(1)) & LZ_DICT_SIZE_MASK];
                cur_match_len = d.dict.b.dict[cur_pos..(cur_pos + lookahead_size)]
                    .iter()
                    .take_while(|&x| *x == c)
                    .count() as u32;
                if cur_match_len < MIN_MATCH_LEN.into() {
                    cur_match_len = 0
                } else {
                    cur_match_dist = 1
                }
            }
        } else {
            // Try to find a match for the bytes at the current position.
            let dist_len = d.dict.find_match(
                lookahead_pos,
                d.dict.size,
                lookahead_size as u32,
                cur_match_dist,
                cur_match_len,
            );
            cur_match_dist = dist_len.0;
            cur_match_len = dist_len.1;
        }

        let far_and_small = cur_match_len == MIN_MATCH_LEN.into() && cur_match_dist >= 8 * 1024;
        let filter_small = d.params.flags & TDEFL_FILTER_MATCHES != 0 && cur_match_len <= 5;
        if far_and_small || filter_small || cur_pos == cur_match_dist as usize {
            cur_match_dist = 0;
            cur_match_len = 0;
        }

        if saved_match_len != 0 {
            if cur_match_len > saved_match_len {
                record_literal(&mut d.huff, &mut d.lz, saved_lit);
                if cur_match_len >= 128 {
                    record_match(&mut d.huff, &mut d.lz, cur_match_len, cur_match_dist);
                    saved_match_len = 0;
                    len_to_move = cur_match_len as usize;
                } else {
                    saved_lit = d.dict.b.dict[cur_pos];
                    saved_match_dist = cur_match_dist;
                    saved_match_len = cur_match_len;
                }
            } else {
                record_match(&mut d.huff, &mut d.lz, saved_match_len, saved_match_dist);
                len_to_move = (saved_match_len - 1) as usize;
                saved_match_len = 0;
            }
        } else if cur_match_dist == 0 {
            record_literal(
                &mut d.huff,
                &mut d.lz,
                d.dict.b.dict[cmp::min(cur_pos, d.dict.b.dict.len() - 1)],
            );
        } else if d.params.greedy_parsing
            || (d.params.flags & TDEFL_RLE_MATCHES != 0)
            || cur_match_len >= 128
        {
            // If we are using lazy matching, check for matches at the next byte if the current
            // match was shorter than 128 bytes.
            record_match(&mut d.huff, &mut d.lz, cur_match_len, cur_match_dist);
            len_to_move = cur_match_len as usize;
        } else {
            saved_lit = d.dict.b.dict[cmp::min(cur_pos, d.dict.b.dict.len() - 1)];
            saved_match_dist = cur_match_dist;
            saved_match_len = cur_match_len;
        }

        lookahead_pos += len_to_move;
        assert!(lookahead_size >= len_to_move);
        lookahead_size -= len_to_move;
        d.dict.size = cmp::min(d.dict.size + len_to_move, LZ_DICT_SIZE);

        let lz_buf_tight = d.lz.code_position > LZ_CODE_BUF_SIZE - 8;
        let fat = ((d.lz.code_position * 115) >> 7) >= d.lz.total_bytes as usize;
        let buf_fat = (d.lz.total_bytes > 31 * 1024) && fat;

        if lz_buf_tight || buf_fat {
            d.params.src_pos = src_pos;
            // These values are used in flush_block, so we need to write them back here.
            d.dict.lookahead_size = lookahead_size;
            d.dict.lookahead_pos = lookahead_pos;

            let n = flush_block(d, callback, TDEFLFlush::None)
                .unwrap_or(TDEFLStatus::PutBufFailed as i32);
            if n != 0 {
                d.params.saved_lit = saved_lit;
                d.params.saved_match_dist = saved_match_dist;
                d.params.saved_match_len = saved_match_len;
                return n > 0;
            }
        }
    }

    d.params.src_pos = src_pos;
    d.dict.lookahead_size = lookahead_size;
    d.dict.lookahead_pos = lookahead_pos;
    d.params.saved_lit = saved_lit;
    d.params.saved_match_dist = saved_match_dist;
    d.params.saved_match_len = saved_match_len;
    true
}

const COMP_FAST_LOOKAHEAD_SIZE: usize = 4096;

fn compress_fast(d: &mut CompressorOxide, callback: &mut CallbackOxide) -> bool {
    let mut src_pos = d.params.src_pos;
    let mut lookahead_size = d.dict.lookahead_size;
    let mut lookahead_pos = d.dict.lookahead_pos;

    let mut cur_pos = lookahead_pos & LZ_DICT_SIZE_MASK;
    let in_buf = match callback.in_buf {
        None => return true,
        Some(in_buf) => in_buf,
    };

    debug_assert!(d.lz.code_position < LZ_CODE_BUF_SIZE - 2);

    while src_pos < in_buf.len() || (d.params.flush != TDEFLFlush::None && lookahead_size > 0) {
        let mut dst_pos = (lookahead_pos + lookahead_size) & LZ_DICT_SIZE_MASK;
        let mut num_bytes_to_process = cmp::min(
            in_buf.len() - src_pos,
            COMP_FAST_LOOKAHEAD_SIZE - lookahead_size,
        );
        lookahead_size += num_bytes_to_process;

        while num_bytes_to_process != 0 {
            let n = cmp::min(LZ_DICT_SIZE - dst_pos, num_bytes_to_process);
            d.dict.b.dict[dst_pos..dst_pos + n].copy_from_slice(&in_buf[src_pos..src_pos + n]);

            if dst_pos < MAX_MATCH_LEN - 1 {
                let m = cmp::min(n, MAX_MATCH_LEN - 1 - dst_pos);
                d.dict.b.dict[dst_pos + LZ_DICT_SIZE..dst_pos + LZ_DICT_SIZE + m]
                    .copy_from_slice(&in_buf[src_pos..src_pos + m]);
            }

            src_pos += n;
            dst_pos = (dst_pos + n) & LZ_DICT_SIZE_MASK;
            num_bytes_to_process -= n;
        }

        d.dict.size = cmp::min(LZ_DICT_SIZE - lookahead_size, d.dict.size);
        if d.params.flush == TDEFLFlush::None && lookahead_size < COMP_FAST_LOOKAHEAD_SIZE {
            break;
        }

        while lookahead_size >= 4 {
            let mut cur_match_len = 1;

            let first_trigram = d.dict.read_unaligned_u32(cur_pos) & 0xFF_FFFF;

            let hash = (first_trigram ^ (first_trigram >> (24 - (LZ_HASH_BITS - 8))))
                & LEVEL1_HASH_SIZE_MASK;

            let mut probe_pos = usize::from(d.dict.b.hash[hash as usize]);
            d.dict.b.hash[hash as usize] = lookahead_pos as u16;

            let mut cur_match_dist = (lookahead_pos - probe_pos) as u16;
            if cur_match_dist as usize <= d.dict.size {
                probe_pos &= LZ_DICT_SIZE_MASK;

                let trigram = d.dict.read_unaligned_u32(probe_pos) & 0xFF_FFFF;

                if first_trigram == trigram {
                    // Trigram was tested, so we can start with "+ 3" displacement.
                    let mut p = cur_pos + 3;
                    let mut q = probe_pos + 3;
                    cur_match_len = (|| {
                        for _ in 0..32 {
                            let p_data: u64 = d.dict.read_unaligned_u64(p);
                            let q_data: u64 = d.dict.read_unaligned_u64(q);
                            let xor_data = p_data ^ q_data;
                            if xor_data == 0 {
                                p += 8;
                                q += 8;
                            } else {
                                let trailing = xor_data.trailing_zeros();
                                return p as u32 - cur_pos as u32 + (trailing >> 3);
                            }
                        }

                        if cur_match_dist == 0 {
                            0
                        } else {
                            MAX_MATCH_LEN as u32
                        }
                    })();

                    if cur_match_len < MIN_MATCH_LEN.into()
                        || (cur_match_len == MIN_MATCH_LEN.into() && cur_match_dist >= 8 * 1024)
                    {
                        let lit = first_trigram as u8;
                        cur_match_len = 1;
                        d.lz.write_code(lit);
                        *d.lz.get_flag() >>= 1;
                        d.huff.count[0][lit as usize] += 1;
                    } else {
                        // Limit the match to the length of the lookahead so we don't create a match
                        // that ends after the end of the input data.
                        cur_match_len = cmp::min(cur_match_len, lookahead_size as u32);
                        debug_assert!(cur_match_len >= MIN_MATCH_LEN.into());
                        debug_assert!(cur_match_len <= MAX_MATCH_LEN as u32);
                        debug_assert!(cur_match_dist >= 1);
                        debug_assert!(cur_match_dist as usize <= LZ_DICT_SIZE);
                        cur_match_dist -= 1;

                        d.lz.write_code((cur_match_len - u32::from(MIN_MATCH_LEN)) as u8);
                        d.lz.write_code(cur_match_dist as u8);
                        d.lz.write_code((cur_match_dist >> 8) as u8);

                        *d.lz.get_flag() >>= 1;
                        *d.lz.get_flag() |= 0x80;
                        if cur_match_dist < 512 {
                            d.huff.count[1][SMALL_DIST_SYM[cur_match_dist as usize] as usize] += 1;
                        } else {
                            d.huff.count[1]
                                [LARGE_DIST_SYM[(cur_match_dist >> 8) as usize] as usize] += 1;
                        }

                        d.huff.count[0][(LEN_SYM
                            [(cur_match_len - u32::from(MIN_MATCH_LEN)) as usize & 255]
                            as usize
                            & 31)
                            + LEN_SYM_OFFSET] += 1;
                    }
                } else {
                    d.lz.write_code(first_trigram as u8);
                    *d.lz.get_flag() >>= 1;
                    d.huff.count[0][first_trigram as u8 as usize] += 1;
                }

                d.lz.consume_flag();
                d.lz.total_bytes += cur_match_len;
                lookahead_pos += cur_match_len as usize;
                d.dict.size = cmp::min(d.dict.size + cur_match_len as usize, LZ_DICT_SIZE);
                cur_pos = (cur_pos + cur_match_len as usize) & LZ_DICT_SIZE_MASK;
                lookahead_size -= cur_match_len as usize;

                if d.lz.code_position > LZ_CODE_BUF_SIZE - 8 {
                    // These values are used in flush_block, so we need to write them back here.
                    d.dict.lookahead_size = lookahead_size;
                    d.dict.lookahead_pos = lookahead_pos;

                    let n = match flush_block(d, callback, TDEFLFlush::None) {
                        Err(_) => {
                            d.params.src_pos = src_pos;
                            d.params.prev_return_status = TDEFLStatus::PutBufFailed;
                            return false;
                        }
                        Ok(status) => status,
                    };
                    if n != 0 {
                        d.params.src_pos = src_pos;
                        return n > 0;
                    }
                    debug_assert!(d.lz.code_position < LZ_CODE_BUF_SIZE - 2);

                    lookahead_size = d.dict.lookahead_size;
                    lookahead_pos = d.dict.lookahead_pos;
                }
            }
        }

        while lookahead_size != 0 {
            let lit = d.dict.b.dict[cur_pos];
            d.lz.total_bytes += 1;
            d.lz.write_code(lit);
            *d.lz.get_flag() >>= 1;
            d.lz.consume_flag();

            d.huff.count[0][lit as usize] += 1;
            lookahead_pos += 1;
            d.dict.size = cmp::min(d.dict.size + 1, LZ_DICT_SIZE);
            cur_pos = (cur_pos + 1) & LZ_DICT_SIZE_MASK;
            lookahead_size -= 1;

            if d.lz.code_position > LZ_CODE_BUF_SIZE - 8 {
                // These values are used in flush_block, so we need to write them back here.
                d.dict.lookahead_size = lookahead_size;
                d.dict.lookahead_pos = lookahead_pos;

                let n = match flush_block(d, callback, TDEFLFlush::None) {
                    Err(_) => {
                        d.params.prev_return_status = TDEFLStatus::PutBufFailed;
                        d.params.src_pos = src_pos;
                        return false;
                    }
                    Ok(status) => status,
                };
                if n != 0 {
                    d.params.src_pos = src_pos;
                    return n > 0;
                }

                lookahead_size = d.dict.lookahead_size;
                lookahead_pos = d.dict.lookahead_pos;
            }
        }
    }

    d.params.src_pos = src_pos;
    d.dict.lookahead_size = lookahead_size;
    d.dict.lookahead_pos = lookahead_pos;
    true
}

fn flush_output_buffer(c: &mut CallbackOxide, p: &mut ParamsOxide) -> (TDEFLStatus, usize, usize) {
    let mut res = (TDEFLStatus::Okay, p.src_pos, 0);
    if let CallbackOut::Buf(ref mut cb) = c.out {
        let n = cmp::min(cb.out_buf.len() - p.out_buf_ofs, p.flush_remaining as usize);
        if n != 0 {
            cb.out_buf[p.out_buf_ofs..p.out_buf_ofs + n]
                .copy_from_slice(&p.local_buf.b[p.flush_ofs as usize..p.flush_ofs as usize + n]);
        }
        p.flush_ofs += n as u32;
        p.flush_remaining -= n as u32;
        p.out_buf_ofs += n;
        res.2 = p.out_buf_ofs;
    }

    if p.finished && p.flush_remaining == 0 {
        res.0 = TDEFLStatus::Done
    }
    res
}

/// Main compression function. Tries to compress as much as possible from `in_buf` and
/// puts compressed output into `out_buf`.
///
/// The value of `flush` determines if the compressor should attempt to flush all output
/// and alternatively try to finish the stream.
///
/// Use [`TDEFLFlush::Finish`] on the final call to signal that the stream is finishing.
///
/// Note that this function does not keep track of whether a flush marker has been output, so
/// if called using [`TDEFLFlush::Sync`], the caller needs to ensure there is enough space in the
/// output buffer if they want to avoid repeated flush markers.
/// See #105 for details.
///
/// # Returns
/// Returns a tuple containing the current status of the compressor, the current position
/// in the input buffer and the current position in the output buffer.
pub fn compress(
    d: &mut CompressorOxide,
    in_buf: &[u8],
    out_buf: &mut [u8],
    flush: TDEFLFlush,
) -> (TDEFLStatus, usize, usize) {
    compress_inner(
        d,
        &mut CallbackOxide::new_callback_buf(in_buf, out_buf),
        flush,
    )
}

/// Main compression function. Callbacks output.
///
/// # Returns
/// Returns a tuple containing the current status of the compressor, the current position
/// in the input buffer.
///
/// The caller is responsible for ensuring the `CallbackFunc` struct will not cause undefined
/// behaviour.
pub fn compress_to_output(
    d: &mut CompressorOxide,
    in_buf: &[u8],
    flush: TDEFLFlush,
    mut callback_func: impl FnMut(&[u8]) -> bool,
) -> (TDEFLStatus, usize) {
    let res = compress_inner(
        d,
        &mut CallbackOxide::new_callback_func(
            in_buf,
            CallbackFunc {
                put_buf_func: &mut callback_func,
            },
        ),
        flush,
    );

    (res.0, res.1)
}

fn compress_inner(
    d: &mut CompressorOxide,
    callback: &mut CallbackOxide,
    flush: TDEFLFlush,
) -> (TDEFLStatus, usize, usize) {
    d.params.out_buf_ofs = 0;
    d.params.src_pos = 0;

    let prev_ok = d.params.prev_return_status == TDEFLStatus::Okay;
    let flush_finish_once = d.params.flush != TDEFLFlush::Finish || flush == TDEFLFlush::Finish;

    d.params.flush = flush;
    if !prev_ok || !flush_finish_once {
        d.params.prev_return_status = TDEFLStatus::BadParam;
        return (d.params.prev_return_status, 0, 0);
    }

    if d.params.flush_remaining != 0 || d.params.finished {
        let res = flush_output_buffer(callback, &mut d.params);
        d.params.prev_return_status = res.0;
        return res;
    }

    let one_probe = d.params.flags & MAX_PROBES_MASK == 1;
    let greedy = d.params.flags & TDEFL_GREEDY_PARSING_FLAG != 0;
    let filter_or_rle = d.params.flags & (TDEFL_FILTER_MATCHES | TDEFL_FORCE_ALL_RAW_BLOCKS) != 0;

    let raw = d.params.flags & TDEFL_FORCE_ALL_RAW_BLOCKS != 0;

    let compress_success = if raw {
        compress_stored(d, callback)
    } else if one_probe && greedy && !filter_or_rle {
        compress_fast(d, callback)
    } else {
        compress_normal(d, callback)
    };

    if !compress_success {
        return (
            d.params.prev_return_status,
            d.params.src_pos,
            d.params.out_buf_ofs,
        );
    }

    if let Some(in_buf) = callback.in_buf {
        if d.params.flags & (TDEFL_WRITE_ZLIB_HEADER | TDEFL_COMPUTE_ADLER32) != 0 {
            d.params.adler32 = update_adler32(d.params.adler32, &in_buf[..d.params.src_pos]);
        }
    }

    let flush_none = d.params.flush == TDEFLFlush::None;
    let in_left = callback.in_buf.map_or(0, |buf| buf.len()) - d.params.src_pos;
    let remaining = in_left != 0 || d.params.flush_remaining != 0;
    if !flush_none && d.dict.lookahead_size == 0 && !remaining {
        let flush = d.params.flush;
        match flush_block(d, callback, flush) {
            Err(_) => {
                d.params.prev_return_status = TDEFLStatus::PutBufFailed;
                return (
                    d.params.prev_return_status,
                    d.params.src_pos,
                    d.params.out_buf_ofs,
                );
            }
            Ok(x) if x < 0 => {
                return (
                    d.params.prev_return_status,
                    d.params.src_pos,
                    d.params.out_buf_ofs,
                )
            }
            _ => {
                d.params.finished = d.params.flush == TDEFLFlush::Finish;
                if d.params.flush == TDEFLFlush::Full {
                    d.dict.b.hash.fill(0);
                    d.dict.b.next.fill(0);
                    d.dict.size = 0;
                }
            }
        }
    }

    let res = flush_output_buffer(callback, &mut d.params);
    d.params.prev_return_status = res.0;

    res
}

/// Create a set of compression flags using parameters used by zlib and other compressors.
/// Mainly intended for use with transition from c libraries as it deals with raw integers.
///
/// # Parameters
/// `level` determines compression level. Clamped to maximum of 10. Negative values result in
/// `CompressionLevel::DefaultLevel`.
/// `window_bits`: Above 0, wraps the stream in a zlib wrapper, 0 or negative for a raw deflate
/// stream.
/// `strategy`: Sets the strategy if this conforms to any of the values in `CompressionStrategy`.
///
/// # Notes
/// This function may be removed or moved to the `miniz_oxide_c_api` in the future.
pub fn create_comp_flags_from_zip_params(level: i32, window_bits: i32, strategy: i32) -> u32 {
    let num_probes = (if level >= 0 {
        cmp::min(10, level)
    } else {
        CompressionLevel::DefaultLevel as i32
    }) as usize;
    let greedy = if level <= 3 {
        TDEFL_GREEDY_PARSING_FLAG
    } else {
        0
    };
    let mut comp_flags = u32::from(NUM_PROBES[num_probes]) | greedy;

    if window_bits > 0 {
        comp_flags |= TDEFL_WRITE_ZLIB_HEADER;
    }

    if level == 0 {
        comp_flags |= TDEFL_FORCE_ALL_RAW_BLOCKS;
    } else if strategy == CompressionStrategy::Filtered as i32 {
        comp_flags |= TDEFL_FILTER_MATCHES;
    } else if strategy == CompressionStrategy::HuffmanOnly as i32 {
        comp_flags &= !MAX_PROBES_MASK;
    } else if strategy == CompressionStrategy::Fixed as i32 {
        comp_flags |= TDEFL_FORCE_ALL_STATIC_BLOCKS;
    } else if strategy == CompressionStrategy::RLE as i32 {
        comp_flags |= TDEFL_RLE_MATCHES;
    }

    comp_flags
}

#[cfg(test)]
mod test {
    use super::{
        compress_to_output, create_comp_flags_from_zip_params, read_u16_le, write_u16_le,
        CompressionStrategy, CompressorOxide, TDEFLFlush, TDEFLStatus, DEFAULT_FLAGS,
        MZ_DEFAULT_WINDOW_BITS,
    };
    use crate::inflate::decompress_to_vec;
    use alloc::vec;

    #[test]
    fn u16_to_slice() {
        let mut slice = [0, 0];
        write_u16_le(2000, &mut slice, 0);
        assert_eq!(slice, [208, 7]);
    }

    #[test]
    fn u16_from_slice() {
        let slice = [208, 7];
        assert_eq!(read_u16_le(&slice, 0), 2000);
    }

    #[test]
    fn compress_output() {
        assert_eq!(
            DEFAULT_FLAGS,
            create_comp_flags_from_zip_params(
                4,
                MZ_DEFAULT_WINDOW_BITS,
                CompressionStrategy::Default as i32
            )
        );

        let slice = [
            1, 2, 3, 4, 1, 2, 3, 1, 2, 3, 1, 2, 6, 1, 2, 3, 1, 2, 3, 2, 3, 1, 2, 3,
        ];
        let mut encoded = vec![];
        let flags = create_comp_flags_from_zip_params(6, 0, 0);
        let mut d = CompressorOxide::new(flags);
        let (status, in_consumed) =
            compress_to_output(&mut d, &slice, TDEFLFlush::Finish, |out: &[u8]| {
                encoded.extend_from_slice(out);
                true
            });

        assert_eq!(status, TDEFLStatus::Done);
        assert_eq!(in_consumed, slice.len());

        let decoded = decompress_to_vec(&encoded[..]).unwrap();
        assert_eq!(&decoded[..], &slice[..]);
    }

    #[test]
    /// Check fast compress mode
    fn compress_fast() {
        let slice = [
            1, 2, 3, 4, 1, 2, 3, 1, 2, 3, 1, 2, 6, 1, 2, 3, 1, 2, 3, 2, 3, 1, 2, 3,
        ];
        let mut encoded = vec![];
        let flags = create_comp_flags_from_zip_params(1, 0, 0);
        let mut d = CompressorOxide::new(flags);
        let (status, in_consumed) =
            compress_to_output(&mut d, &slice, TDEFLFlush::Finish, |out: &[u8]| {
                encoded.extend_from_slice(out);
                true
            });

        assert_eq!(status, TDEFLStatus::Done);
        assert_eq!(in_consumed, slice.len());

        // Needs to be altered if algorithm improves.
        assert_eq!(
            &encoded[..],
            [99, 100, 98, 102, 1, 98, 48, 98, 3, 147, 204, 76, 204, 140, 76, 204, 0]
        );

        let decoded = decompress_to_vec(&encoded[..]).unwrap();
        assert_eq!(&decoded[..], &slice[..]);
    }

    #[test]
    fn zlib_window_bits() {
        use crate::inflate::stream::{inflate, InflateState};
        use crate::DataFormat;
        use alloc::boxed::Box;
        let slice = [
            1, 2, 3, 4, 1, 2, 3, 1, 2, 3, 1, 2, 6, 1, 2, 3, 1, 2, 3, 2, 3, 1, 2, 3, 35, 22, 22, 2,
            6, 2, 6,
        ];
        let mut encoded = vec![];
        let flags = create_comp_flags_from_zip_params(2, 1, CompressionStrategy::RLE.into());
        let mut d = CompressorOxide::new(flags);
        let (status, in_consumed) =
            compress_to_output(&mut d, &slice, TDEFLFlush::Finish, |out: &[u8]| {
                encoded.extend_from_slice(out);
                true
            });

        assert_eq!(status, TDEFLStatus::Done);
        assert_eq!(in_consumed, slice.len());

        let mut output = vec![0; slice.len()];

        let mut decompressor = Box::new(InflateState::new(DataFormat::Zlib));

        let mut out_slice = output.as_mut_slice();
        // Feed 1 byte at a time and no back buffer to test that RLE encoding has been used.
        for i in 0..encoded.len() {
            let result = inflate(
                &mut decompressor,
                &encoded[i..i + 1],
                out_slice,
                crate::MZFlush::None,
            );
            out_slice = &mut out_slice[result.bytes_written..];
        }
        let cmf = decompressor.decompressor().zlib_header().0;
        assert_eq!(cmf, 8);
        assert_eq!(output, slice)
    }
}
