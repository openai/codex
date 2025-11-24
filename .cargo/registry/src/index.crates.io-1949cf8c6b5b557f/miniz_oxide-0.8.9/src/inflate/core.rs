//! Streaming decompression functionality.

use super::*;
use crate::shared::{update_adler32, HUFFMAN_LENGTH_ORDER};
use ::core::cell::Cell;

use ::core::cmp;
use ::core::convert::TryInto;

use self::output_buffer::{InputWrapper, OutputBuffer};

#[cfg(feature = "serde")]
use crate::serde::big_array::BigArray;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub const TINFL_LZ_DICT_SIZE: usize = 32_768;

/// A struct containing huffman code lengths and the huffman code tree used by the decompressor.
#[cfg_attr(not(feature = "rustc-dep-of-std"), derive(Clone))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct HuffmanTable {
    /// Fast lookup table for shorter huffman codes.
    ///
    /// See `HuffmanTable::fast_lookup`.
    #[cfg_attr(feature = "serde", serde(with = "BigArray"))]
    pub look_up: [i16; FAST_LOOKUP_SIZE as usize],
    /// Full huffman tree.
    ///
    /// Positive values are edge nodes/symbols, negative values are
    /// parent nodes/references to other nodes.
    #[cfg_attr(feature = "serde", serde(with = "BigArray"))]
    pub tree: [i16; MAX_HUFF_TREE_SIZE],
}

impl HuffmanTable {
    const fn new() -> HuffmanTable {
        HuffmanTable {
            look_up: [0; FAST_LOOKUP_SIZE as usize],
            tree: [0; MAX_HUFF_TREE_SIZE],
        }
    }

    /// Look for a symbol in the fast lookup table.
    /// The symbol is stored in the lower 9 bits, the length in the next 6.
    /// If the returned value is negative, the code wasn't found in the
    /// fast lookup table and the full tree has to be traversed to find the code.
    #[inline]
    fn fast_lookup(&self, bit_buf: BitBuffer) -> i16 {
        self.look_up[(bit_buf & BitBuffer::from(FAST_LOOKUP_SIZE - 1)) as usize]
    }

    /// Get the symbol and the code length from the huffman tree.
    #[inline]
    fn tree_lookup(&self, fast_symbol: i32, bit_buf: BitBuffer, mut code_len: u8) -> (i32, u32) {
        let mut symbol = fast_symbol;
        // We step through the tree until we encounter a positive value, which indicates a
        // symbol.
        loop {
            // symbol here indicates the position of the left (0) node, if the next bit is 1
            // we add 1 to the lookup position to get the right node.
            let tree_index = (!symbol + ((bit_buf >> code_len) & 1) as i32) as usize;

            // Use get here to avoid generatic panic code.
            // The init_tree code should prevent this from actually going out of bounds
            // but if there were somehow a bug with that
            // we would at worst end up with corrupted output in release mode.
            debug_assert!(tree_index < self.tree.len());
            symbol = i32::from(self.tree.get(tree_index).copied().unwrap_or(i16::MAX));
            code_len += 1;
            if symbol >= 0 {
                break;
            }
        }
        // Note: Using a u8 for code_len inside this function seems to improve performance, but changing it
        // in localvars seems to worsen things so we convert it to a u32 here.
        (symbol, u32::from(code_len))
    }

    #[inline]
    /// Look up a symbol and code length from the bits in the provided bit buffer.
    ///
    /// Returns Some(symbol, length) on success,
    /// None if the length is 0.
    ///
    /// It's possible we could avoid checking for 0 if we can guarantee a sane table.
    /// TODO: Check if a smaller type for code_len helps performance.
    fn lookup(&self, bit_buf: BitBuffer) -> (i32, u32) {
        let symbol = self.fast_lookup(bit_buf).into();
        if symbol >= 0 {
            let length = (symbol >> 9) as u32;
            (symbol, length)
        } else {
            // We didn't get a symbol from the fast lookup table, so check the tree instead.
            self.tree_lookup(symbol, bit_buf, FAST_LOOKUP_BITS)
        }
    }
}

/// The number of huffman tables used.
const MAX_HUFF_TABLES: usize = 3;
/// The length of the first (literal/length) huffman table.
const MAX_HUFF_SYMBOLS_0: usize = 288;
/// The length of the second (distance) huffman table.
const MAX_HUFF_SYMBOLS_1: usize = 32;
/// The length of the last (huffman code length) huffman table.
const MAX_HUFF_SYMBOLS_2: usize = 19;
/// The maximum length of a code that can be looked up in the fast lookup table.
const FAST_LOOKUP_BITS: u8 = 10;
/// The size of the fast lookup table.
const FAST_LOOKUP_SIZE: u16 = 1 << FAST_LOOKUP_BITS;
const MAX_HUFF_TREE_SIZE: usize = MAX_HUFF_SYMBOLS_0 * 2;
const LITLEN_TABLE: usize = 0;
const DIST_TABLE: usize = 1;
const HUFFLEN_TABLE: usize = 2;
const LEN_CODES_SIZE: usize = 512;
const LEN_CODES_MASK: usize = LEN_CODES_SIZE - 1;

/// Flags to [`decompress()`] to control how inflation works.
///
/// These define bits for a bitmask argument.
pub mod inflate_flags {
    /// Should we try to parse a zlib header?
    ///
    /// If unset, the function will expect an RFC1951 deflate stream.  If set, it will expect a
    /// RFC1950 zlib wrapper around the deflate stream.
    pub const TINFL_FLAG_PARSE_ZLIB_HEADER: u32 = 1;

    /// There will be more input that hasn't been given to the decompressor yet.
    ///
    /// This is useful when you want to decompress what you have so far,
    /// even if you know there is probably more input that hasn't gotten here yet (_e.g._, over a
    /// network connection).  When [`decompress()`][super::decompress] reaches the end of the input
    /// without finding the end of the compressed stream, it will return
    /// [`TINFLStatus::NeedsMoreInput`][super::TINFLStatus::NeedsMoreInput] if this is set,
    /// indicating that you should get more data before calling again.  If not set, it will return
    /// [`TINFLStatus::FailedCannotMakeProgress`][super::TINFLStatus::FailedCannotMakeProgress]
    /// suggesting the stream is corrupt, since you claimed it was all there.
    pub const TINFL_FLAG_HAS_MORE_INPUT: u32 = 2;

    /// The output buffer should not wrap around.
    pub const TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF: u32 = 4;

    /// Calculate the adler32 checksum of the output data even if we're not inflating a zlib stream.
    ///
    /// If [`TINFL_FLAG_IGNORE_ADLER32`] is specified, it will override this.
    ///
    /// NOTE: Enabling/disabling this between calls to decompress will result in an incorrect
    /// checksum.
    pub const TINFL_FLAG_COMPUTE_ADLER32: u32 = 8;

    /// Ignore adler32 checksum even if we are inflating a zlib stream.
    ///
    /// Overrides [`TINFL_FLAG_COMPUTE_ADLER32`] if both are enabled.
    ///
    /// NOTE: This flag does not exist in miniz as it does not support this and is a
    /// custom addition for miniz_oxide.
    ///
    /// NOTE: Should not be changed from enabled to disabled after decompression has started,
    /// this will result in checksum failure (outside the unlikely event where the checksum happens
    /// to match anyway).
    pub const TINFL_FLAG_IGNORE_ADLER32: u32 = 64;

    /// Return [`TINFLStatus::BlockBoundary`][super::TINFLStatus::BlockBoundary]
    /// on reaching the boundary between deflate blocks. Calling [`decompress()`][super::decompress]
    /// again will resume decompression of the next block.
    #[cfg(feature = "block-boundary")]
    pub const TINFL_FLAG_STOP_ON_BLOCK_BOUNDARY: u32 = 128;
}

use self::inflate_flags::*;

const MIN_TABLE_SIZES: [u16; 3] = [257, 1, 4];

#[cfg(target_pointer_width = "64")]
type BitBuffer = u64;

#[cfg(not(target_pointer_width = "64"))]
type BitBuffer = u32;

/*
enum HuffmanTableType {
    LiteralLength = 0,
    Dist = 1,
    Huffman = 2,
}*/

/// Minimal data representing the [`DecompressorOxide`] state when it is between deflate blocks
/// (i.e. [`decompress()`] has returned [`TINFLStatus::BlockBoundary`]).
/// This can be serialized along with the last 32KiB of the output buffer, then passed to
/// [`DecompressorOxide::from_block_boundary_state()`] to resume decompression from the same point.
///
/// The Zlib/Adler32 fields can be ignored if you aren't using those features
/// ([`TINFL_FLAG_PARSE_ZLIB_HEADER`], [`TINFL_FLAG_COMPUTE_ADLER32`]).
/// When deserializing, you can reconstruct `bit_buf` from the previous byte in the input file
/// (if you still have access to it), so `num_bits` is the only field that is always required.
#[derive(Clone)]
#[cfg(feature = "block-boundary")]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BlockBoundaryState {
    /// The number of bits from the last byte of input consumed,
    /// that are needed for decoding the next deflate block.
    /// Value is in range `0..=7`
    pub num_bits: u8,

    /// The `num_bits` MSBs from the last byte of input consumed,
    /// that are needed for decoding the next deflate block.
    /// Stored in the LSBs of this field.
    pub bit_buf: u8,

    /// Zlib CMF
    pub z_header0: u32,
    /// Zlib FLG
    pub z_header1: u32,
    /// Adler32 checksum of the data decompressed so far
    pub check_adler32: u32,
}

#[cfg(feature = "block-boundary")]
impl Default for BlockBoundaryState {
    fn default() -> Self {
        BlockBoundaryState {
            num_bits: 0,
            bit_buf: 0,
            z_header0: 0,
            z_header1: 0,
            check_adler32: 1,
        }
    }
}

/// Main decompression struct.
///
#[cfg_attr(not(feature = "rustc-dep-of-std"), derive(Clone))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DecompressorOxide {
    /// Current state of the decompressor.
    state: core::State,
    /// Number of bits in the bit buffer.
    num_bits: u32,
    /// Zlib CMF
    z_header0: u32,
    /// Zlib FLG
    z_header1: u32,
    /// Adler32 checksum from the zlib header.
    z_adler32: u32,
    /// 1 if the current block is the last block, 0 otherwise.
    finish: u8,
    /// The type of the current block.
    /// or if in a dynamic block, which huffman table we are currently
    // initializing.
    block_type: u8,
    /// 1 if the adler32 value should be checked.
    check_adler32: u32,
    /// Last match distance.
    dist: u32,
    /// Variable used for match length, symbols, and a number of other things.
    counter: u32,
    /// Number of extra bits for the last length or distance code.
    num_extra: u8,
    /// Number of entries in each huffman table.
    table_sizes: [u16; MAX_HUFF_TABLES],
    /// Buffer of input data.
    bit_buf: BitBuffer,
    /// Huffman tables.
    tables: [HuffmanTable; MAX_HUFF_TABLES],

    #[cfg_attr(feature = "serde", serde(with = "BigArray"))]
    code_size_literal: [u8; MAX_HUFF_SYMBOLS_0],
    code_size_dist: [u8; MAX_HUFF_SYMBOLS_1],
    code_size_huffman: [u8; MAX_HUFF_SYMBOLS_2],
    /// Raw block header.
    raw_header: [u8; 4],
    /// Huffman length codes.
    #[cfg_attr(feature = "serde", serde(with = "BigArray"))]
    // MAX_HUFF_SYMBOLS_0 + MAX_HUFF_SYMBOLS_1 + 137
    // Extended to 512 to allow masking to help evade bounds checks.
    len_codes: [u8; LEN_CODES_SIZE],
}

impl DecompressorOxide {
    /// Create a new tinfl_decompressor with all fields set to 0.
    pub fn new() -> DecompressorOxide {
        DecompressorOxide::default()
    }

    /// Set the current state to `Start`.
    #[inline]
    pub fn init(&mut self) {
        // The rest of the data is reset or overwritten when used.
        self.state = core::State::Start;
    }

    /// Returns the adler32 checksum of the currently decompressed data.
    /// Note: Will return Some(1) if decompressing zlib but ignoring adler32.
    #[inline]
    #[cfg(not(feature = "rustc-dep-of-std"))]
    pub fn adler32(&self) -> Option<u32> {
        if self.state != State::Start && !self.state.is_failure() && self.z_header0 != 0 {
            Some(self.check_adler32)
        } else {
            None
        }
    }

    /// Returns the adler32 that was read from the zlib header if it exists.
    #[inline]
    #[cfg(not(feature = "rustc-dep-of-std"))]
    pub fn adler32_header(&self) -> Option<u32> {
        if self.state != State::Start && self.state != State::BadZlibHeader && self.z_header0 != 0 {
            Some(self.z_adler32)
        } else {
            None
        }
    }

    // Get zlib header for tests
    // Only for tests for now, may provide a proper function for this for later.
    #[cfg(all(test, feature = "with-alloc"))]
    pub(crate) const fn zlib_header(&self) -> (u32, u32) {
        (self.z_header0, self.z_header1)
    }

    /*fn code_size_table(&mut self, table_num: u8) -> &mut [u8] {
        match table_num {
            0 => &mut self.code_size_literal,
            1 => &mut self.code_size_dist,
            _ => &mut self.code_size_huffman,
        }
    }*/

    /// Returns the current [`BlockBoundaryState`]. Should only be called when
    /// [`decompress()`] has returned [`TINFLStatus::BlockBoundary`];
    /// otherwise this will return `None`.
    #[cfg(feature = "block-boundary")]
    pub fn block_boundary_state(&self) -> Option<BlockBoundaryState> {
        if self.state == core::State::ReadBlockHeader {
            // If we're in this state, undo_bytes should have emptied
            // bit_buf of any whole bytes
            assert!(self.num_bits < 8);

            Some(BlockBoundaryState {
                num_bits: self.num_bits as u8,
                bit_buf: self.bit_buf as u8,
                z_header0: self.z_header0,
                z_header1: self.z_header1,
                check_adler32: self.check_adler32,
            })
        } else {
            None
        }
    }

    /// Creates a new `DecompressorOxide` from the state returned by
    /// `block_boundary_state()`.
    ///
    /// When calling [`decompress()`], the 32KiB of `out` preceding `out_pos` must be
    /// initialized with the same data that it contained when `block_boundary_state()`
    /// was called.
    #[cfg(feature = "block-boundary")]
    pub fn from_block_boundary_state(st: &BlockBoundaryState) -> Self {
        DecompressorOxide {
            state: core::State::ReadBlockHeader,
            num_bits: st.num_bits as u32,
            bit_buf: st.bit_buf as BitBuffer,
            z_header0: st.z_header0,
            z_header1: st.z_header1,
            z_adler32: 1,
            check_adler32: st.check_adler32,
            ..DecompressorOxide::default()
        }
    }
}

impl Default for DecompressorOxide {
    /// Create a new tinfl_decompressor with all fields set to 0.
    #[inline(always)]
    fn default() -> Self {
        DecompressorOxide {
            state: core::State::Start,
            num_bits: 0,
            z_header0: 0,
            z_header1: 0,
            z_adler32: 0,
            finish: 0,
            block_type: 0,
            check_adler32: 0,
            dist: 0,
            counter: 0,
            num_extra: 0,
            table_sizes: [0; MAX_HUFF_TABLES],
            bit_buf: 0,
            // TODO:(oyvindln) Check that copies here are optimized out in release mode.
            tables: [
                HuffmanTable::new(),
                HuffmanTable::new(),
                HuffmanTable::new(),
            ],
            code_size_literal: [0; MAX_HUFF_SYMBOLS_0],
            code_size_dist: [0; MAX_HUFF_SYMBOLS_1],
            code_size_huffman: [0; MAX_HUFF_SYMBOLS_2],
            raw_header: [0; 4],
            len_codes: [0; LEN_CODES_SIZE],
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
enum State {
    Start = 0,
    ReadZlibCmf,
    ReadZlibFlg,
    ReadBlockHeader,
    BlockTypeNoCompression,
    RawHeader,
    RawMemcpy1,
    RawMemcpy2,
    ReadTableSizes,
    ReadHufflenTableCodeSize,
    ReadLitlenDistTablesCodeSize,
    ReadExtraBitsCodeSize,
    DecodeLitlen,
    WriteSymbol,
    ReadExtraBitsLitlen,
    DecodeDistance,
    ReadExtraBitsDistance,
    RawReadFirstByte,
    RawStoreFirstByte,
    WriteLenBytesToEnd,
    BlockDone,
    HuffDecodeOuterLoop1,
    HuffDecodeOuterLoop2,
    ReadAdler32,

    DoneForever,

    // Failure states.
    BlockTypeUnexpected,
    BadCodeSizeSum,
    BadDistOrLiteralTableLength,
    BadTotalSymbols,
    BadZlibHeader,
    DistanceOutOfBounds,
    BadRawLength,
    BadCodeSizeDistPrevLookup,
    InvalidLitlen,
    InvalidDist,
}

impl State {
    #[cfg(not(feature = "rustc-dep-of-std"))]
    const fn is_failure(self) -> bool {
        matches!(
            self,
            BlockTypeUnexpected
                | BadCodeSizeSum
                | BadDistOrLiteralTableLength
                | BadTotalSymbols
                | BadZlibHeader
                | DistanceOutOfBounds
                | BadRawLength
                | BadCodeSizeDistPrevLookup
                | InvalidLitlen
                | InvalidDist
        )
    }

    #[inline]
    fn begin(&mut self, new_state: State) {
        *self = new_state;
    }
}

use self::State::*;

// # Optimization
// We add a extra value at the end and make the tables 32 elements long
// so we can use a mask to avoid bounds checks.
// The invalid values are set to something high enough to avoid underflowing
// the match length.
/// Base length for each length code.
///
/// The base is used together with the value of the extra bits to decode the actual
/// length/distance values in a match.
#[rustfmt::skip]
const LENGTH_BASE: [u16; 32] = [
    3,  4,  5,  6,  7,  8,  9,  10,  11,  13,  15,  17,  19,  23,  27,  31,
    35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258, 512, 512, 512
];

/// Number of extra bits for each length code.
#[rustfmt::skip]
const LENGTH_EXTRA: [u8; 32] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2,
    3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0, 0, 0, 0
];

/// Base length for each distance code.
#[rustfmt::skip]
const DIST_BASE: [u16; 30] = [
    1,    2,    3,    4,    5,    7,      9,      13,     17,     25,    33,
    49,   65,   97,   129,  193,  257,    385,    513,    769,    1025,  1537,
    2049, 3073, 4097, 6145, 8193, 12_289, 16_385, 24_577
];

/// Get the number of extra bits used for a distance code.
/// (Code numbers above `NUM_DISTANCE_CODES` will give some garbage
/// value.)
#[inline(always)]
const fn num_extra_bits_for_distance_code(code: u8) -> u8 {
    // TODO: Need to verify that this is faster on all platforms.
    // This can be easily calculated without a lookup.
    let c = code >> 1;
    c.saturating_sub(1)
}

/// The mask used when indexing the base/extra arrays.
const BASE_EXTRA_MASK: usize = 32 - 1;

/// Read an le u16 value from the slice iterator.
///
/// # Panics
/// Panics if there are less than two bytes left.
#[inline]
fn read_u16_le(iter: &mut InputWrapper) -> u16 {
    let ret = {
        let two_bytes = iter.as_slice()[..2].try_into().unwrap_or_default();
        u16::from_le_bytes(two_bytes)
    };
    iter.advance(2);
    ret
}

/// Ensure that there is data in the bit buffer.
///
/// On 64-bit platform, we use a 64-bit value so this will
/// result in there being at least 32 bits in the bit buffer.
/// This function assumes that there is at least 4 bytes left in the input buffer.
#[inline(always)]
#[cfg(target_pointer_width = "64")]
fn fill_bit_buffer(l: &mut LocalVars, in_iter: &mut InputWrapper) {
    // Read four bytes into the buffer at once.
    if l.num_bits < 30 {
        l.bit_buf |= BitBuffer::from(in_iter.read_u32_le()) << l.num_bits;
        l.num_bits += 32;
    }
}

/// Same as previous, but for non-64-bit platforms.
/// Ensures at least 16 bits are present, requires at least 2 bytes in the in buffer.
#[inline(always)]
#[cfg(not(target_pointer_width = "64"))]
fn fill_bit_buffer(l: &mut LocalVars, in_iter: &mut InputWrapper) {
    // If the buffer is 32-bit wide, read 2 bytes instead.
    if l.num_bits < 15 {
        l.bit_buf |= BitBuffer::from(read_u16_le(in_iter)) << l.num_bits;
        l.num_bits += 16;
    }
}

/// Check that the zlib header is correct and that there is enough space in the buffer
/// for the window size specified in the header.
///
/// See https://tools.ietf.org/html/rfc1950
#[inline]
const fn validate_zlib_header(cmf: u32, flg: u32, flags: u32, mask: usize) -> Action {
    let mut failed =
    // cmf + flg should be divisible by 31.
        (((cmf * 256) + flg) % 31 != 0) ||
    // If this flag is set, a dictionary was used for this zlib compressed data.
    // This is currently not supported by miniz or miniz-oxide
        ((flg & 0b0010_0000) != 0) ||
    // Compression method. Only 8(DEFLATE) is defined by the standard.
        ((cmf & 15) != 8);

    let window_size = 1 << ((cmf >> 4) + 8);
    if (flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF) == 0 {
        // Bail if the buffer is wrapping and the window size is larger than the buffer.
        failed |= (mask + 1) < window_size;
    }

    // Zlib doesn't allow window sizes above 32 * 1024.
    failed |= window_size > 32_768;

    if failed {
        Action::Jump(BadZlibHeader)
    } else {
        Action::Jump(ReadBlockHeader)
    }
}

enum Action {
    None,
    Jump(State),
    End(TINFLStatus),
}

/// Try to decode the next huffman code, and puts it in the counter field of the decompressor
/// if successful.
///
/// # Returns
/// The specified action returned from `f` on success,
/// `Action::End` if there are not enough data left to decode a symbol.
fn decode_huffman_code<F>(
    r: &mut DecompressorOxide,
    l: &mut LocalVars,
    table: usize,
    flags: u32,
    in_iter: &mut InputWrapper,
    f: F,
) -> Action
where
    F: FnOnce(&mut DecompressorOxide, &mut LocalVars, i32) -> Action,
{
    // As the huffman codes can be up to 15 bits long we need at least 15 bits
    // ready in the bit buffer to start decoding the next huffman code.
    if l.num_bits < 15 {
        // First, make sure there is enough data in the bit buffer to decode a huffman code.
        if in_iter.bytes_left() < 2 {
            // If there is less than 2 bytes left in the input buffer, we try to look up
            // the huffman code with what's available, and return if that doesn't succeed.
            // Original explanation in miniz:
            // /* TINFL_HUFF_BITBUF_FILL() is only used rarely, when the number of bytes
            //  * remaining in the input buffer falls below 2. */
            // /* It reads just enough bytes from the input stream that are needed to decode
            //  * the next Huffman code (and absolutely no more). It works by trying to fully
            //  * decode a */
            // /* Huffman code by using whatever bits are currently present in the bit buffer.
            //  * If this fails, it reads another byte, and tries again until it succeeds or
            //  * until the */
            // /* bit buffer contains >=15 bits (deflate's max. Huffman code size). */
            loop {
                let mut temp = i32::from(r.tables[table].fast_lookup(l.bit_buf));
                if temp >= 0 {
                    let code_len = (temp >> 9) as u32;
                    // TODO: Is there any point to check for code_len != 0 here still?
                    if (code_len != 0) && (l.num_bits >= code_len) {
                        break;
                    }
                } else if l.num_bits > FAST_LOOKUP_BITS.into() {
                    let mut code_len = u32::from(FAST_LOOKUP_BITS);
                    loop {
                        temp = i32::from(
                            r.tables[table].tree
                                [(!temp + ((l.bit_buf >> code_len) & 1) as i32) as usize],
                        );
                        code_len += 1;
                        if temp >= 0 || l.num_bits < code_len + 1 {
                            break;
                        }
                    }
                    if temp >= 0 {
                        break;
                    }
                }

                // TODO: miniz jumps straight to here after getting here again after failing to read
                // a byte.
                // Doing that lets miniz avoid re-doing the lookup that that was done in the
                // previous call.
                let mut byte = 0;
                if let a @ Action::End(_) = read_byte(in_iter, flags, |b| {
                    byte = b;
                    Action::None
                }) {
                    return a;
                };

                // Do this outside closure for now to avoid borrowing r.
                l.bit_buf |= BitBuffer::from(byte) << l.num_bits;
                l.num_bits += 8;

                if l.num_bits >= 15 {
                    break;
                }
            }
        } else {
            // There is enough data in the input buffer, so read the next two bytes
            // and add them to the bit buffer.
            // Unwrapping here is fine since we just checked that there are at least two
            // bytes left.
            l.bit_buf |= BitBuffer::from(read_u16_le(in_iter)) << l.num_bits;
            l.num_bits += 16;
        }
    }

    // We now have at least 15 bits in the input buffer.
    let mut symbol = i32::from(r.tables[table].fast_lookup(l.bit_buf));
    let code_len;
    // If the symbol was found in the fast lookup table.
    if symbol >= 0 {
        // Get the length value from the top bits.
        // As we shift down the sign bit, converting to an unsigned value
        // shouldn't overflow.
        code_len = (symbol >> 9) as u32;
        // Mask out the length value.
        symbol &= 511;
    } else {
        let res = r.tables[table].tree_lookup(symbol, l.bit_buf, FAST_LOOKUP_BITS);
        symbol = res.0;
        code_len = res.1;
    };

    l.bit_buf >>= code_len;
    l.num_bits -= code_len;
    f(r, l, symbol)
}

/// Try to read one byte from `in_iter` and call `f` with the read byte as an argument,
/// returning the result.
/// If reading fails, `Action::End is returned`
#[inline]
fn read_byte<F>(in_iter: &mut InputWrapper, flags: u32, f: F) -> Action
where
    F: FnOnce(u8) -> Action,
{
    match in_iter.read_byte() {
        None => end_of_input(flags),
        Some(byte) => f(byte),
    }
}

// TODO: `l: &mut LocalVars` may be slow similar to decompress_fast (even with inline(always))
/// Try to read `amount` number of bits from `in_iter` and call the function `f` with the bits as an
/// an argument after reading, returning the result of that function, or `Action::End` if there are
/// not enough bytes left.
#[inline]
#[allow(clippy::while_immutable_condition)]
fn read_bits<F>(
    l: &mut LocalVars,
    amount: u32,
    in_iter: &mut InputWrapper,
    flags: u32,
    f: F,
) -> Action
where
    F: FnOnce(&mut LocalVars, BitBuffer) -> Action,
{
    // Clippy gives a false positive warning here due to the closure.
    // Read enough bytes from the input iterator to cover the number of bits we want.
    while l.num_bits < amount {
        let action = read_byte(in_iter, flags, |byte| {
            l.bit_buf |= BitBuffer::from(byte) << l.num_bits;
            l.num_bits += 8;
            Action::None
        });

        // If there are not enough bytes in the input iterator, return and signal that we need more.
        if !matches!(action, Action::None) {
            return action;
        }
    }

    let bits = l.bit_buf & ((1 << amount) - 1);
    l.bit_buf >>= amount;
    l.num_bits -= amount;
    f(l, bits)
}

#[inline]
fn pad_to_bytes<F>(l: &mut LocalVars, in_iter: &mut InputWrapper, flags: u32, f: F) -> Action
where
    F: FnOnce(&mut LocalVars) -> Action,
{
    let num_bits = l.num_bits & 7;
    read_bits(l, num_bits, in_iter, flags, |l, _| f(l))
}

#[inline]
const fn end_of_input(flags: u32) -> Action {
    Action::End(if flags & TINFL_FLAG_HAS_MORE_INPUT != 0 {
        TINFLStatus::NeedsMoreInput
    } else {
        TINFLStatus::FailedCannotMakeProgress
    })
}

#[inline]
fn undo_bytes(l: &mut LocalVars, max: u32) -> u32 {
    let res = cmp::min(l.num_bits >> 3, max);
    l.num_bits -= res << 3;
    res
}

fn start_static_table(r: &mut DecompressorOxide) {
    r.table_sizes[LITLEN_TABLE] = 288;
    r.table_sizes[DIST_TABLE] = 32;
    r.code_size_literal[0..144].fill(8);
    r.code_size_literal[144..256].fill(9);
    r.code_size_literal[256..280].fill(7);
    r.code_size_literal[280..288].fill(8);
    r.code_size_dist[0..32].fill(5);
}

#[cfg(any(
    feature = "rustc-dep-of-std",
    not(feature = "with-alloc"),
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "loongarch64"
))]
#[inline]
const fn reverse_bits(n: u16) -> u16 {
    // Lookup is not used when building as part of std to avoid wasting space
    // for lookup table in every rust binary
    // as it's only used for backtraces in the cold path
    // - see #152

    // armv7 and newer, and loongarch have a cpu instruction for bit reversal so
    // it's preferable to just use that on those architectures.

    // Also disable lookup table when not using the alloc feature as
    // we probably don't want to waste space for a lookup table in an environment
    // without an allocator.
    n.reverse_bits()
}

#[cfg(all(
    not(any(
        feature = "rustc-dep-of-std",
        target_arch = "aarch64",
        target_arch = "arm64ec",
        target_arch = "loongarch64"
    )),
    feature = "with-alloc"
))]
fn reverse_bits(n: u16) -> u16 {
    static REVERSED_BITS_LOOKUP: [u16; 512] = {
        let mut table = [0; 512];

        let mut i = 0;
        while i < 512 {
            table[i] = (i as u16).reverse_bits();
            i += 1;
        }

        table
    };

    REVERSED_BITS_LOOKUP[n as usize]
}

fn init_tree(r: &mut DecompressorOxide, l: &mut LocalVars) -> Option<Action> {
    loop {
        let bt = r.block_type as usize;

        let code_sizes = match bt {
            LITLEN_TABLE => &mut r.code_size_literal[..],
            DIST_TABLE => &mut r.code_size_dist,
            HUFFLEN_TABLE => &mut r.code_size_huffman,
            _ => return None,
        };
        let table = &mut r.tables[bt];

        let mut total_symbols = [0u16; 16];
        // Next code - we use the odd length here to simplify a loop later.
        let mut next_code = [0u32; 17];
        const INVALID_CODE: i16 = (1 << 9) | 286;
        // Set the values in the fast table to return a
        // non-zero length and an invalid symbol instead of zero
        // so that we do not have to have a check for a zero
        // code length in the hot code path later
        // and can instead error out on the invalid symbol check
        // on bogus input.
        table.look_up.fill(INVALID_CODE);
        // If we are initializing the huffman code length we can skip
        // this since these codes can't be longer than 3 bits
        // and thus only use the fast table and this table won't be accessed so
        // there is no point clearing it.
        // TODO: Avoid creating this table at all.
        if bt != HUFFLEN_TABLE {
            table.tree.fill(0);
        }

        let table_size = r.table_sizes[bt] as usize;
        if table_size > code_sizes.len() {
            return None;
        }

        for &code_size in &code_sizes[..table_size] {
            let cs = code_size as usize;
            // Code sizes are limited to max 15 according to the
            // deflate spec.
            // If it is larger than this, something has gone wrong...
            if cs >= total_symbols.len() {
                return None;
            }
            total_symbols[cs] += 1;
        }

        let mut used_symbols = 0;
        let mut total = 0u32;
        // Count up the total number of used lengths and check that the table is not under or over-subscribed.
        for (&ts, next) in total_symbols.iter().zip(next_code[1..].iter_mut()).skip(1) {
            used_symbols += ts;
            total += u32::from(ts);
            total <<= 1;
            *next = total;
        }

        //
        // While it's not explicitly stated in the spec, a hufflen table
        // with a single length (or none) would be invalid as there needs to be
        // at minimum a length for both a non-zero length huffman code for the end of block symbol
        // and one of the codes to represent 0 to make sense - so just reject that here as well.
        //
        // The distance table is allowed to have a single distance code though according to the spect it is
        // supposed to be accompanied by a second dummy code. It can also be empty indicating no used codes.
        //
        // The literal/length table can not be empty as there has to be an end of block symbol,
        // The standard doesn't specify that there should be a dummy code in case of a single
        // symbol (i.e an empty block). Normally that's not an issue though the code will have
        // to take that into account later on in case of malformed input.
        if total != 65_536 && (used_symbols > 1 || bt == HUFFLEN_TABLE) {
            return Some(Action::Jump(BadTotalSymbols));
        }

        let mut tree_next = -1;
        for symbol_index in 0..table_size {
            // Code sizes are limited to 15 according to the spec
            // It's already checked earlier but the compiler might not be smart enough to know that.
            let code_size = code_sizes[symbol_index] & 15;
            if code_size == 0 {
                continue;
            }

            let cur_code = next_code[code_size as usize];
            next_code[code_size as usize] += 1;

            let n = (cur_code & (u32::MAX >> (32 - code_size))) as u16;

            let mut rev_code = if n < 512 {
                // Using a lookup table
                // for a small speedup here,
                // Seems to only really make a difference on very short
                // inputs however.
                // 512 seems to be around a sweet spot.
                reverse_bits(n)
            } else {
                n.reverse_bits()
            } >> (16 - code_size);

            if code_size <= FAST_LOOKUP_BITS {
                let k = (i16::from(code_size) << 9) | symbol_index as i16;
                while rev_code < FAST_LOOKUP_SIZE {
                    table.look_up[rev_code as usize] = k;
                    rev_code += 1 << code_size;
                }
                continue;
            }

            let mut tree_cur = table.look_up[(rev_code & (FAST_LOOKUP_SIZE - 1)) as usize];
            if tree_cur == INVALID_CODE {
                table.look_up[(rev_code & (FAST_LOOKUP_SIZE - 1)) as usize] = tree_next;
                tree_cur = tree_next;
                tree_next -= 2;
            }

            rev_code >>= FAST_LOOKUP_BITS - 1;
            for _ in FAST_LOOKUP_BITS + 1..code_size {
                rev_code >>= 1;
                tree_cur -= (rev_code & 1) as i16;
                let tree_index = (-tree_cur - 1) as usize;
                if tree_index >= table.tree.len() {
                    return None;
                }
                if table.tree[tree_index] == 0 {
                    table.tree[tree_index] = tree_next;
                    tree_cur = tree_next;
                    tree_next -= 2;
                } else {
                    tree_cur = table.tree[tree_index];
                }
            }

            rev_code >>= 1;
            tree_cur -= (rev_code & 1) as i16;
            let tree_index = (-tree_cur - 1) as usize;
            if tree_index >= table.tree.len() {
                return None;
            }
            table.tree[tree_index] = symbol_index as i16;
        }

        if r.block_type == HUFFLEN_TABLE as u8 {
            l.counter = 0;
            return Some(Action::Jump(ReadLitlenDistTablesCodeSize));
        }

        if r.block_type == LITLEN_TABLE as u8 {
            break;
        }
        r.block_type -= 1;
    }

    l.counter = 0;

    Some(Action::Jump(DecodeLitlen))
}

// A helper macro for generating the state machine.
//
// As Rust doesn't have fallthrough on matches, we have to return to the match statement
// and jump for each state change. (Which would ideally be optimized away, but often isn't.)
macro_rules! generate_state {
    ($state: ident, $state_machine: tt, $f: expr) => {
        loop {
            match $f {
                Action::None => continue,
                Action::Jump(new_state) => {
                    $state = new_state;
                    continue $state_machine;
                },
                Action::End(result) => break $state_machine result,
            }
        }
    };
}

#[derive(Copy, Clone)]
struct LocalVars {
    pub bit_buf: BitBuffer,
    pub num_bits: u32,
    pub dist: u32,
    pub counter: u32,
    pub num_extra: u8,
}

#[inline]
fn transfer(
    out_slice: &mut [u8],
    mut source_pos: usize,
    mut out_pos: usize,
    match_len: usize,
    out_buf_size_mask: usize,
) {
    // special case that comes up surprisingly often. in the case that `source_pos`
    // is 1 less than `out_pos`, we can say that the entire range will be the same
    // value and optimize this to be a simple `memset`
    let source_diff = if source_pos > out_pos {
        source_pos - out_pos
    } else {
        out_pos - source_pos
    };

    // The last 3 bytes can wrap as those are dealt with separately at the end.
    // Use wrapping_sub rather than saturating for performance reasons here as
    // if source_pos + match_len  is < 3 we just want to jump to the end
    // condition anyhow.
    let not_wrapping = (out_buf_size_mask == usize::MAX)
        || ((source_pos + match_len).wrapping_sub(3) < out_slice.len());

    let end_pos = ((match_len >> 2) * 4) + out_pos;
    if not_wrapping && source_diff == 1 && out_pos > source_pos {
        let end = (match_len >> 2) * 4 + out_pos;
        let init = out_slice[out_pos - 1];
        out_slice[out_pos..end].fill(init);
        out_pos = end;
        source_pos = end - 1;
    // if the difference between `source_pos` and `out_pos` is greater than 3,
    // and we are not wrapping, we
    // can do slightly better than the naive case by copying everything at once
    } else if not_wrapping && out_pos > source_pos && (out_pos - source_pos >= 4) {
        let end_pos = cmp::min(end_pos, out_slice.len().saturating_sub(3));
        while out_pos < end_pos {
            out_slice.copy_within(source_pos..=source_pos + 3, out_pos);
            source_pos += 4;
            out_pos += 4;
        }
    } else {
        let end_pos = cmp::min(end_pos, out_slice.len().saturating_sub(3));
        while out_pos < end_pos {
            // Placing these assertions moves some bounds check before the accesses which
            // makes the compiler able to optimize better.
            // Ideally we would find a safe way to remove them entirely.
            assert!(out_pos + 3 < out_slice.len());
            assert!((source_pos + 3) & out_buf_size_mask < out_slice.len());

            out_slice[out_pos] = out_slice[source_pos & out_buf_size_mask];
            out_slice[out_pos + 1] = out_slice[(source_pos + 1) & out_buf_size_mask];
            out_slice[out_pos + 2] = out_slice[(source_pos + 2) & out_buf_size_mask];
            out_slice[out_pos + 3] = out_slice[(source_pos + 3) & out_buf_size_mask];
            source_pos += 4;
            out_pos += 4;
        }
    }

    match match_len & 3 {
        0 => (),
        1 => out_slice[out_pos] = out_slice[source_pos & out_buf_size_mask],
        2 => {
            assert!(out_pos + 1 < out_slice.len());
            assert!((source_pos + 1) & out_buf_size_mask < out_slice.len());
            out_slice[out_pos] = out_slice[source_pos & out_buf_size_mask];
            out_slice[out_pos + 1] = out_slice[(source_pos + 1) & out_buf_size_mask];
        }
        3 => {
            assert!(out_pos + 2 < out_slice.len());
            assert!((source_pos + 2) & out_buf_size_mask < out_slice.len());
            out_slice[out_pos] = out_slice[source_pos & out_buf_size_mask];
            out_slice[out_pos + 1] = out_slice[(source_pos + 1) & out_buf_size_mask];
            out_slice[out_pos + 2] = out_slice[(source_pos + 2) & out_buf_size_mask];
        }
        _ => unreachable!(),
    }
}

/// Presumes that there is at least match_len bytes in output left.
#[inline]
fn apply_match(
    out_slice: &mut [u8],
    out_pos: usize,
    dist: usize,
    match_len: usize,
    out_buf_size_mask: usize,
) {
    debug_assert!(out_pos.checked_add(match_len).unwrap() <= out_slice.len());

    let source_pos = out_pos.wrapping_sub(dist) & out_buf_size_mask;

    if match_len == 3 {
        let out_slice = Cell::from_mut(out_slice).as_slice_of_cells();
        if let Some(dst) = out_slice.get(out_pos..out_pos + 3) {
            // Moving bounds checks before any memory mutation allows the optimizer
            // combine them together.
            let src = out_slice
                .get(source_pos)
                .zip(out_slice.get((source_pos + 1) & out_buf_size_mask))
                .zip(out_slice.get((source_pos + 2) & out_buf_size_mask));
            if let Some(((a, b), c)) = src {
                // For correctness, the memory reads and writes have to be interleaved.
                // Cells make it possible for read and write references to overlap.
                dst[0].set(a.get());
                dst[1].set(b.get());
                dst[2].set(c.get());
            }
        }
        return;
    }

    if cfg!(not(any(target_arch = "x86", target_arch = "x86_64"))) {
        // The copy from slice code seems to not give any added performance at least on
        // armv7 so transfer manually
        // Need to test on other platforms.
        transfer(out_slice, source_pos, out_pos, match_len, out_buf_size_mask);
        return;
    }

    if source_pos >= out_pos && (source_pos - out_pos) < match_len {
        transfer(out_slice, source_pos, out_pos, match_len, out_buf_size_mask);
    } else if match_len <= dist && source_pos + match_len < out_slice.len() {
        // Destination and source segments does not intersect and source does not wrap.
        // TODO: An invalid before start of data wrapping match reached here before
        // it was fixed (it wrapped around and ended overlapping again)- need
        // to check that we are not wrapping here.
        if source_pos < out_pos {
            let (from_slice, to_slice) = out_slice.split_at_mut(out_pos);
            to_slice[..match_len].copy_from_slice(&from_slice[source_pos..source_pos + match_len]);
        } else {
            let (to_slice, from_slice) = out_slice.split_at_mut(source_pos);
            to_slice[out_pos..out_pos + match_len].copy_from_slice(&from_slice[..match_len]);
        }
    } else {
        transfer(out_slice, source_pos, out_pos, match_len, out_buf_size_mask);
    }
}

/// Fast inner decompression loop which is run  while there is at least
/// 259 bytes left in the output buffer, and at least 6 bytes left in the input buffer
/// (The maximum one match would need + 1).
///
/// This was inspired by a similar optimization in zlib, which uses this info to do
/// faster unchecked copies of multiple bytes at a time.
/// Currently we don't do this here, but this function does avoid having to jump through the
/// big match loop on each state change(as rust does not have fallthrough or gotos at the moment),
/// and already improves decompression speed a fair bit.
fn decompress_fast(
    r: &mut DecompressorOxide,
    in_iter: &mut InputWrapper,
    out_buf: &mut OutputBuffer,
    flags: u32,
    local_vars: &mut LocalVars,
    out_buf_size_mask: usize,
) -> (TINFLStatus, State) {
    // Make a local copy of the most used variables, to avoid having to update and read from values
    // in a random memory location and to encourage more register use.
    let mut l = *local_vars;
    let mut state;

    let status: TINFLStatus = 'o: loop {
        state = State::DecodeLitlen;
        loop {
            // This function assumes that there is at least 259 bytes left in the output buffer,
            // and that there is at least 14 bytes left in the input buffer. 14 input bytes:
            // 15 (prev lit) + 15 (length) + 5 (length extra) + 15 (dist)
            // + 29 + 32 (left in bit buf, including last 13 dist extra) = 111 bits < 14 bytes
            // We need the one extra byte as we may write one length and one full match
            // before checking again.
            if out_buf.bytes_left() < 259 || in_iter.bytes_left() < 14 {
                state = State::DecodeLitlen;
                break 'o TINFLStatus::Done;
            }

            fill_bit_buffer(&mut l, in_iter);

            let (symbol, code_len) = r.tables[LITLEN_TABLE].lookup(l.bit_buf);
            l.counter = symbol as u32;
            l.bit_buf >>= code_len;
            l.num_bits -= code_len;

            if (l.counter & 256) != 0 {
                // The symbol is not a literal.
                break;
            } else {
                // If we have a 32-bit buffer we need to read another two bytes now
                // to have enough bits to keep going.
                if cfg!(not(target_pointer_width = "64")) {
                    fill_bit_buffer(&mut l, in_iter);
                }

                let (symbol, code_len) = r.tables[LITLEN_TABLE].lookup(l.bit_buf);
                l.bit_buf >>= code_len;
                l.num_bits -= code_len;
                // The previous symbol was a literal, so write it directly and check
                // the next one.
                out_buf.write_byte(l.counter as u8);
                if (symbol & 256) != 0 {
                    l.counter = symbol as u32;
                    // The symbol is a length value.
                    break;
                } else {
                    // The symbol is a literal, so write it directly and continue.
                    out_buf.write_byte(symbol as u8);
                }
            }
        }

        // Mask the top bits since they may contain length info.
        l.counter &= 511;
        if l.counter == 256 {
            // We hit the end of block symbol.
            state.begin(BlockDone);
            break 'o TINFLStatus::Done;
        } else if l.counter > 285 {
            // Invalid code.
            // We already verified earlier that the code is > 256.
            state.begin(InvalidLitlen);
            break 'o TINFLStatus::Failed;
        } else {
            // The symbol was a length code.
            // # Optimization
            // Mask the value to avoid bounds checks
            // While the maximum is checked, the compiler isn't able to know that the
            // value won't wrap around here.
            l.num_extra = LENGTH_EXTRA[(l.counter - 257) as usize & BASE_EXTRA_MASK];
            l.counter = u32::from(LENGTH_BASE[(l.counter - 257) as usize & BASE_EXTRA_MASK]);
            // Length and distance codes have a number of extra bits depending on
            // the base, which together with the base gives us the exact value.

            // We need to make sure we have at least 33 (so min 5 bytes) bits in the buffer at this spot.
            fill_bit_buffer(&mut l, in_iter);
            if l.num_extra != 0 {
                let extra_bits = l.bit_buf & ((1 << l.num_extra) - 1);
                l.bit_buf >>= l.num_extra;
                l.num_bits -= u32::from(l.num_extra);
                l.counter += extra_bits as u32;
            }

            // We found a length code, so a distance code should follow.

            if cfg!(not(target_pointer_width = "64")) {
                fill_bit_buffer(&mut l, in_iter);
            }

            let (mut symbol, code_len) = r.tables[DIST_TABLE].lookup(l.bit_buf);
            symbol &= 511;
            l.bit_buf >>= code_len;
            l.num_bits -= code_len;
            if symbol > 29 {
                state.begin(InvalidDist);
                break 'o TINFLStatus::Failed;
            }

            l.num_extra = num_extra_bits_for_distance_code(symbol as u8);
            l.dist = u32::from(DIST_BASE[symbol as usize]);

            if l.num_extra != 0 {
                fill_bit_buffer(&mut l, in_iter);
                let extra_bits = l.bit_buf & ((1 << l.num_extra) - 1);
                l.bit_buf >>= l.num_extra;
                l.num_bits -= u32::from(l.num_extra);
                l.dist += extra_bits as u32;
            }

            let position = out_buf.position();
            if (l.dist as usize > out_buf.position()
                && (flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0))
                || (l.dist as usize > out_buf.get_ref().len())
            {
                // We encountered a distance that refers a position before
                // the start of the decoded data, so we can't continue.
                state.begin(DistanceOutOfBounds);
                break TINFLStatus::Failed;
            }

            apply_match(
                out_buf.get_mut(),
                position,
                l.dist as usize,
                l.counter as usize,
                out_buf_size_mask,
            );

            out_buf.set_position(position + l.counter as usize);
        }
    };

    *local_vars = l;
    (status, state)
}

/// Main decompression function. Keeps decompressing data from `in_buf` until the `in_buf` is
/// empty, `out` is full, the end of the deflate stream is hit, or there is an error in the
/// deflate stream.
///
/// # Arguments
///
/// `r` is a [`DecompressorOxide`] struct with the state of this stream.
///
/// `in_buf` is a reference to the compressed data that is to be decompressed. The decompressor will
/// start at the first byte of this buffer.
///
/// `out` is a reference to the buffer that will store the decompressed data, and that
/// stores previously decompressed data if any.
///
/// * The offset given by `out_pos` indicates where in the output buffer slice writing should start.
/// * If [`TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF`] is not set, the output buffer is used in a
///   wrapping manner, and it's size is required to be a power of 2.
/// * The decompression function normally needs access to 32KiB of the previously decompressed data
///   (or to the beginning of the decompressed data if less than 32KiB has been decompressed.)
///     - If this data is not available, decompression may fail.
///     - Some deflate compressors allow specifying a window size which limits match distances to
///       less than this, or alternatively an RLE mode where matches will only refer to the previous byte
///       and thus allows a smaller output buffer. The window size can be specified in the zlib
///       header structure, however, the header data should not be relied on to be correct.
///
/// `flags` indicates settings and status to the decompression function.
/// * The [`TINFL_FLAG_HAS_MORE_INPUT`] has to be specified if more compressed data is to be provided
///   in a subsequent call to this function.
/// * See the the [`inflate_flags`] module for details on other flags.
///
/// # Returns
///
/// Returns a tuple containing the status of the compressor, the number of input bytes read, and the
/// number of bytes output to `out`.
pub fn decompress(
    r: &mut DecompressorOxide,
    in_buf: &[u8],
    out: &mut [u8],
    out_pos: usize,
    flags: u32,
) -> (TINFLStatus, usize, usize) {
    let out_buf_size_mask = if flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0 {
        usize::MAX
    } else {
        // In the case of zero len, any attempt to write would produce HasMoreOutput,
        // so to gracefully process the case of there really being no output,
        // set the mask to all zeros.
        out.len().saturating_sub(1)
    };

    // Ensure the output buffer's size is a power of 2, unless the output buffer
    // is large enough to hold the entire output file (in which case it doesn't
    // matter).
    // Also make sure that the output buffer position is not past the end of the output buffer.
    if (out_buf_size_mask.wrapping_add(1) & out_buf_size_mask) != 0 || out_pos > out.len() {
        return (TINFLStatus::BadParam, 0, 0);
    }

    let mut in_iter = InputWrapper::from_slice(in_buf);

    let mut state = r.state;

    let mut out_buf = OutputBuffer::from_slice_and_pos(out, out_pos);

    // Make a local copy of the important variables here so we can work with them on the stack.
    let mut l = LocalVars {
        bit_buf: r.bit_buf,
        num_bits: r.num_bits,
        dist: r.dist,
        counter: r.counter,
        num_extra: r.num_extra,
    };

    let mut status = 'state_machine: loop {
        match state {
            Start => generate_state!(state, 'state_machine, {
                l.bit_buf = 0;
                l.num_bits = 0;
                l.dist = 0;
                l.counter = 0;
                l.num_extra = 0;
                r.z_header0 = 0;
                r.z_header1 = 0;
                r.z_adler32 = 1;
                r.check_adler32 = 1;
                if flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0 {
                    Action::Jump(State::ReadZlibCmf)
                } else {
                    Action::Jump(State::ReadBlockHeader)
                }
            }),

            ReadZlibCmf => generate_state!(state, 'state_machine, {
                read_byte(&mut in_iter, flags, |cmf| {
                    r.z_header0 = u32::from(cmf);
                    Action::Jump(State::ReadZlibFlg)
                })
            }),

            ReadZlibFlg => generate_state!(state, 'state_machine, {
                read_byte(&mut in_iter, flags, |flg| {
                    r.z_header1 = u32::from(flg);
                    validate_zlib_header(r.z_header0, r.z_header1, flags, out_buf_size_mask)
                })
            }),

            // Read the block header and jump to the relevant section depending on the block type.
            ReadBlockHeader => generate_state!(state, 'state_machine, {
                read_bits(&mut l, 3, &mut in_iter, flags, |l, bits| {
                    r.finish = (bits & 1) as u8;
                    r.block_type = ((bits >> 1) & 3) as u8;
                    match r.block_type {
                        0 => Action::Jump(BlockTypeNoCompression),
                        1 => {
                            start_static_table(r);
                            init_tree(r, l).unwrap_or(Action::End(TINFLStatus::Failed))
                        },
                        2 => {
                            l.counter = 0;
                            Action::Jump(ReadTableSizes)
                        },
                        3 => Action::Jump(BlockTypeUnexpected),
                        _ => unreachable!()
                    }
                })
            }),

            // Raw/Stored/uncompressed block.
            BlockTypeNoCompression => generate_state!(state, 'state_machine, {
                pad_to_bytes(&mut l, &mut in_iter, flags, |l| {
                    l.counter = 0;
                    Action::Jump(RawHeader)
                })
            }),

            // Check that the raw block header is correct.
            RawHeader => generate_state!(state, 'state_machine, {
                if l.counter < 4 {
                    // Read block length and block length check.
                    if l.num_bits != 0 {
                        read_bits(&mut l, 8, &mut in_iter, flags, |l, bits| {
                            r.raw_header[l.counter as usize] = bits as u8;
                            l.counter += 1;
                            Action::None
                        })
                    } else {
                        read_byte(&mut in_iter, flags, |byte| {
                            r.raw_header[l.counter as usize] = byte;
                            l.counter += 1;
                            Action::None
                        })
                    }
                } else {
                    // Check if the length value of a raw block is correct.
                    // The 2 first (2-byte) words in a raw header are the length and the
                    // ones complement of the length.
                    let length = u16::from(r.raw_header[0]) | (u16::from(r.raw_header[1]) << 8);
                    let check = u16::from(r.raw_header[2]) | (u16::from(r.raw_header[3]) << 8);
                    let valid = length == !check;
                    l.counter = length.into();

                    if !valid {
                        Action::Jump(BadRawLength)
                    } else if l.counter == 0 {
                        // Empty raw block. Sometimes used for synchronization.
                        Action::Jump(BlockDone)
                    } else if l.num_bits != 0 {
                        // There is some data in the bit buffer, so we need to write that first.
                        Action::Jump(RawReadFirstByte)
                    } else {
                        // The bit buffer is empty, so memcpy the rest of the uncompressed data from
                        // the block.
                        Action::Jump(RawMemcpy1)
                    }
                }
            }),

            // Read the byte from the bit buffer.
            RawReadFirstByte => generate_state!(state, 'state_machine, {
                read_bits(&mut l, 8, &mut in_iter, flags, |l, bits| {
                    l.dist = bits as u32;
                    Action::Jump(RawStoreFirstByte)
                })
            }),

            // Write the byte we just read to the output buffer.
            RawStoreFirstByte => generate_state!(state, 'state_machine, {
                if out_buf.bytes_left() == 0 {
                    Action::End(TINFLStatus::HasMoreOutput)
                } else {
                    out_buf.write_byte(l.dist as u8);
                    l.counter -= 1;
                    if l.counter == 0 || l.num_bits == 0 {
                        Action::Jump(RawMemcpy1)
                    } else {
                        // There is still some data left in the bit buffer that needs to be output.
                        // TODO: Changed this to jump to `RawReadfirstbyte` rather than
                        // `RawStoreFirstByte` as that seemed to be the correct path, but this
                        // needs testing.
                        Action::Jump(RawReadFirstByte)
                    }
                }
            }),

            RawMemcpy1 => generate_state!(state, 'state_machine, {
                if l.counter == 0 {
                    Action::Jump(BlockDone)
                } else if out_buf.bytes_left() == 0 {
                    Action::End(TINFLStatus::HasMoreOutput)
                } else {
                    Action::Jump(RawMemcpy2)
                }
            }),

            RawMemcpy2 => generate_state!(state, 'state_machine, {
                if in_iter.bytes_left() > 0 {
                    // Copy as many raw bytes as possible from the input to the output using memcpy.
                    // Raw block lengths are limited to 64 * 1024, so casting through usize and u32
                    // is not an issue.
                    let space_left = out_buf.bytes_left();
                    let bytes_to_copy = cmp::min(cmp::min(
                        space_left,
                        in_iter.bytes_left()),
                        l.counter as usize
                    );

                    out_buf.write_slice(&in_iter.as_slice()[..bytes_to_copy]);

                    in_iter.advance(bytes_to_copy);
                    l.counter -= bytes_to_copy as u32;
                    Action::Jump(RawMemcpy1)
                } else {
                    end_of_input(flags)
                }
            }),

            // Read how many huffman codes/symbols are used for each table.
            ReadTableSizes => generate_state!(state, 'state_machine, {
                if l.counter < 3 {
                    let num_bits = [5, 5, 4][l.counter as usize];
                    read_bits(&mut l, num_bits, &mut in_iter, flags, |l, bits| {
                        r.table_sizes[l.counter as usize] =
                            bits as u16 + MIN_TABLE_SIZES[l.counter as usize];
                        l.counter += 1;
                        Action::None
                    })
                } else {
                    r.code_size_huffman.fill(0);
                    l.counter = 0;
                    // Check that the litlen and distance are within spec.
                    // litlen table should be <=286 acc to the RFC and
                    // additionally zlib rejects dist table sizes larger than 30.
                    // NOTE this the final sizes after adding back predefined values, not
                    // raw value in the data.
                    // See miniz_oxide issue #130 and https://github.com/madler/zlib/issues/82.
                    if r.table_sizes[LITLEN_TABLE] <= 286 && r.table_sizes[DIST_TABLE] <= 30 {
                        Action::Jump(ReadHufflenTableCodeSize)
                    }
                    else {
                        Action::Jump(BadDistOrLiteralTableLength)
                    }
                }
            }),

            // Read the 3-bit lengths of the huffman codes describing the huffman code lengths used
            // to decode the lengths of the main tables.
            ReadHufflenTableCodeSize => generate_state!(state, 'state_machine, {
                if l.counter < r.table_sizes[HUFFLEN_TABLE].into() {
                    read_bits(&mut l, 3, &mut in_iter, flags, |l, bits| {
                        // These lengths are not stored in a normal ascending order, but rather one
                        // specified by the deflate specification intended to put the most used
                        // values at the front as trailing zero lengths do not have to be stored.
                        r.code_size_huffman[HUFFMAN_LENGTH_ORDER[l.counter as usize] as usize] =
                                bits as u8;
                        l.counter += 1;
                        Action::None
                    })
                } else {
                    r.table_sizes[HUFFLEN_TABLE] = MAX_HUFF_SYMBOLS_2 as u16;
                    init_tree(r, &mut l).unwrap_or(Action::End(TINFLStatus::Failed))
                }
            }),

            ReadLitlenDistTablesCodeSize => generate_state!(state, 'state_machine, {
                if l.counter < u32::from(r.table_sizes[LITLEN_TABLE]) + u32::from(r.table_sizes[DIST_TABLE]) {
                    decode_huffman_code(
                        r, &mut l, HUFFLEN_TABLE,
                        flags, &mut in_iter, |r, l, symbol| {
                            l.dist = symbol as u32;
                            if l.dist < 16 {
                                r.len_codes[l.counter as usize & LEN_CODES_MASK] = l.dist as u8;
                                l.counter += 1;
                                Action::None
                            } else if l.dist == 16 && l.counter == 0 {
                                Action::Jump(BadCodeSizeDistPrevLookup)
                            } else {
                                // Last value is a dummy to allow mask.
                                l.num_extra = [2, 3, 7, 0][(l.dist as usize - 16) & 3];
                                Action::Jump(ReadExtraBitsCodeSize)
                            }
                        }
                    )
                } else if l.counter != u32::from(r.table_sizes[LITLEN_TABLE]) + u32::from(r.table_sizes[DIST_TABLE]) {
                    Action::Jump(BadCodeSizeSum)
                } else {

                    r.code_size_literal[..r.table_sizes[LITLEN_TABLE] as usize]
                        .copy_from_slice(&r.len_codes[..r.table_sizes[LITLEN_TABLE] as usize & LEN_CODES_MASK]);

                    let dist_table_start = r.table_sizes[LITLEN_TABLE] as usize;
                    debug_assert!(dist_table_start < r.len_codes.len());
                    let dist_table_end = (r.table_sizes[LITLEN_TABLE] +
                                          r.table_sizes[DIST_TABLE]) as usize;
                    let code_size_dist_end = r.table_sizes[DIST_TABLE] as usize;
                    debug_assert!(dist_table_end < r.len_codes.len());
                    debug_assert!(code_size_dist_end < r.code_size_dist.len());
                    let dist_table_start = dist_table_start & LEN_CODES_MASK;
                    let dist_table_end = dist_table_end & LEN_CODES_MASK;
                    r.code_size_dist[..code_size_dist_end & (MAX_HUFF_SYMBOLS_1 - 1)]
                        .copy_from_slice(&r.len_codes[dist_table_start..dist_table_end]);

                    r.block_type -= 1;
                    init_tree(r, &mut l).unwrap_or(Action::End(TINFLStatus::Failed))
                }
            }),

            ReadExtraBitsCodeSize => generate_state!(state, 'state_machine, {
                let num_extra = l.num_extra.into();
                read_bits(&mut l, num_extra, &mut in_iter, flags, |l, mut extra_bits| {
                    // Mask to avoid a bounds check.
                    // We can use 2 since the 2 first values are the same.
                    extra_bits += [3, 3, 11][(l.dist as usize - 16) & 2];
                    let val = if l.dist == 16 {
                        debug_assert!(l.counter as usize - 1 < r.len_codes.len());
                        r.len_codes[(l.counter as usize - 1) & LEN_CODES_MASK]
                    } else {
                        0
                    };

                    let fill_start = l.counter as usize;
                    let fill_end = l.counter as usize + extra_bits as usize;
                    debug_assert!(fill_start < r.len_codes.len());
                    debug_assert!(fill_end < r.len_codes.len());

                    r.len_codes[
                            fill_start & LEN_CODES_MASK..fill_end & LEN_CODES_MASK
                        ].fill(val);
                    l.counter += extra_bits as u32;
                    Action::Jump(ReadLitlenDistTablesCodeSize)
                })
            }),

            DecodeLitlen => generate_state!(state, 'state_machine, {
                if in_iter.bytes_left() < 4 || out_buf.bytes_left() < 2 {
                    // See if we can decode a literal with the data we have left.
                    // Jumps to next state (WriteSymbol) if successful.
                    decode_huffman_code(
                        r,
                        &mut l,
                        LITLEN_TABLE,
                        flags,
                        &mut in_iter,
                        |_r, l, symbol| {
                            l.counter = symbol as u32;
                            Action::Jump(WriteSymbol)
                        },
                    )
                } else if
                // If there is enough space, use the fast inner decompression
                // function.
                    out_buf.bytes_left() >= 259 &&
                    in_iter.bytes_left() >= 14
                {
                    let (status, new_state) = decompress_fast(
                        r,
                        &mut in_iter,
                        &mut out_buf,
                        flags,
                        &mut l,
                        out_buf_size_mask,
                    );

                    state = new_state;
                    if status == TINFLStatus::Done {
                        Action::Jump(new_state)
                    } else {
                        Action::End(status)
                    }
                } else {
                    fill_bit_buffer(&mut l, &mut in_iter);

                    let (symbol, code_len) = r.tables[LITLEN_TABLE].lookup(l.bit_buf);

                    l.counter = symbol as u32;
                    l.bit_buf >>= code_len;
                    l.num_bits -= code_len;

                    if (l.counter & 256) != 0 {
                        // The symbol is not a literal.
                        Action::Jump(HuffDecodeOuterLoop1)
                    } else {
                        // If we have a 32-bit buffer we need to read another two bytes now
                        // to have enough bits to keep going.
                        if cfg!(not(target_pointer_width = "64")) {
                            fill_bit_buffer(&mut l, &mut in_iter);
                        }

                        let (symbol, code_len) = r.tables[LITLEN_TABLE].lookup(l.bit_buf);

                            l.bit_buf >>= code_len;
                            l.num_bits -= code_len;
                            // The previous symbol was a literal, so write it directly and check
                            // the next one.
                            out_buf.write_byte(l.counter as u8);
                            if (symbol & 256) != 0 {
                                l.counter = symbol as u32;
                                // The symbol is a length value.
                                Action::Jump(HuffDecodeOuterLoop1)
                            } else {
                                // The symbol is a literal, so write it directly and continue.
                                out_buf.write_byte(symbol as u8);
                                Action::None
                            }

                    }

                }
            }),

            WriteSymbol => generate_state!(state, 'state_machine, {
                if l.counter >= 256 {
                    Action::Jump(HuffDecodeOuterLoop1)
                } else if out_buf.bytes_left() > 0 {
                    out_buf.write_byte(l.counter as u8);
                    Action::Jump(DecodeLitlen)
                } else {
                    Action::End(TINFLStatus::HasMoreOutput)
                }
            }),

            HuffDecodeOuterLoop1 => generate_state!(state, 'state_machine, {
                // Mask the top bits since they may contain length info.
                l.counter &= 511;

                if l.counter
                    == 256 {
                    // We hit the end of block symbol.
                    Action::Jump(BlockDone)
                } else if l.counter > 285 {
                    // Invalid code.
                    // We already verified earlier that the code is > 256.
                    Action::Jump(InvalidLitlen)
                } else {
                    // # Optimization
                    // Mask the value to avoid bounds checks
                    // We could use get_unchecked later if can statically verify that
                    // this will never go out of bounds.
                    l.num_extra =
                        LENGTH_EXTRA[(l.counter - 257) as usize & BASE_EXTRA_MASK];
                    l.counter = u32::from(LENGTH_BASE[(l.counter - 257) as usize & BASE_EXTRA_MASK]);
                    // Length and distance codes have a number of extra bits depending on
                    // the base, which together with the base gives us the exact value.
                    if l.num_extra != 0 {
                        Action::Jump(ReadExtraBitsLitlen)
                    } else {
                        Action::Jump(DecodeDistance)
                    }
                }
            }),

            ReadExtraBitsLitlen => generate_state!(state, 'state_machine, {
                let num_extra = l.num_extra.into();
                read_bits(&mut l, num_extra, &mut in_iter, flags, |l, extra_bits| {
                    l.counter += extra_bits as u32;
                    Action::Jump(DecodeDistance)
                })
            }),

            DecodeDistance => generate_state!(state, 'state_machine, {
                // Try to read a huffman code from the input buffer and look up what
                // length code the decoded symbol refers to.
                decode_huffman_code(r, &mut l, DIST_TABLE, flags, &mut in_iter, |_r, l, symbol| {
                    // # Optimizaton - transform the value into usize here before the check so
                    // the compiler can optimize the bounds check later - ideally it should
                    // know that the value can't be negative from earlier in the
                    // decode_huffman_code function but it seems it may not be able
                    // to make the assumption that it can't be negative and thus
                    // overflow if it's converted after the check.
                    let symbol = symbol as usize;
                    if symbol > 29 {
                        // Invalid distance code.
                        return Action::Jump(InvalidDist)
                    }
                    l.num_extra = num_extra_bits_for_distance_code(symbol as u8);
                    l.dist = u32::from(DIST_BASE[symbol]);
                    if l.num_extra != 0 {
                        // ReadEXTRA_BITS_DISTACNE
                        Action::Jump(ReadExtraBitsDistance)
                    } else {
                        Action::Jump(HuffDecodeOuterLoop2)
                    }
                })
            }),

            ReadExtraBitsDistance => generate_state!(state, 'state_machine, {
                let num_extra = l.num_extra.into();
                read_bits(&mut l, num_extra, &mut in_iter, flags, |l, extra_bits| {
                    l.dist += extra_bits as u32;
                    Action::Jump(HuffDecodeOuterLoop2)
                })
            }),

            HuffDecodeOuterLoop2 => generate_state!(state, 'state_machine, {
                if (l.dist as usize > out_buf.position() &&
                    (flags & TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF != 0)) || (l.dist as usize > out_buf.get_ref().len())
                {
                    // We encountered a distance that refers a position before
                    // the start of the decoded data, so we can't continue.
                    Action::Jump(DistanceOutOfBounds)
                } else {
                    let out_pos = out_buf.position();
                    let source_pos = out_buf.position()
                        .wrapping_sub(l.dist as usize) & out_buf_size_mask;

                    let out_len = out_buf.get_ref().len();
                    let match_end_pos = out_buf.position() + l.counter as usize;

                    if match_end_pos > out_len ||
                        // miniz doesn't do this check here. Not sure how it makes sure
                        // that this case doesn't happen.
                        (source_pos >= out_pos && (source_pos - out_pos) < l.counter as usize)
                    {
                        // Not enough space for all of the data in the output buffer,
                        // so copy what we have space for.
                        if l.counter == 0 {
                            Action::Jump(DecodeLitlen)
                        } else {
                            Action::Jump(WriteLenBytesToEnd)
                        }
                    } else {
                        apply_match(
                            out_buf.get_mut(),
                            out_pos,
                            l.dist as usize,
                            l.counter as usize,
                            out_buf_size_mask
                        );
                        out_buf.set_position(out_pos + l.counter as usize);
                        Action::Jump(DecodeLitlen)
                    }
                }
            }),

            WriteLenBytesToEnd => generate_state!(state, 'state_machine, {
                if out_buf.bytes_left() > 0 {
                    let out_pos = out_buf.position();
                    let source_pos = out_buf.position()
                        .wrapping_sub(l.dist as usize) & out_buf_size_mask;


                    let len = cmp::min(out_buf.bytes_left(), l.counter as usize);

                    transfer(out_buf.get_mut(), source_pos, out_pos, len, out_buf_size_mask);

                    out_buf.set_position(out_pos + len);
                    l.counter -= len as u32;
                    if l.counter == 0 {
                        Action::Jump(DecodeLitlen)
                    } else {
                        Action::None
                    }
                } else {
                    Action::End(TINFLStatus::HasMoreOutput)
                }
            }),

            BlockDone => generate_state!(state, 'state_machine, {
                // End once we've read the last block.
                if r.finish != 0 {
                    pad_to_bytes(&mut l, &mut in_iter, flags, |_| Action::None);

                    let in_consumed = in_buf.len() - in_iter.bytes_left();
                    let undo = undo_bytes(&mut l, in_consumed as u32) as usize;
                    in_iter = InputWrapper::from_slice(in_buf[in_consumed - undo..].iter().as_slice());

                    l.bit_buf &= ((1 as BitBuffer) << l.num_bits) - 1;
                    debug_assert_eq!(l.num_bits, 0);

                    if flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0 {
                        l.counter = 0;
                        Action::Jump(ReadAdler32)
                    } else {
                        Action::Jump(DoneForever)
                    }
                } else {
                    #[cfg(feature = "block-boundary")]
                    if flags & TINFL_FLAG_STOP_ON_BLOCK_BOUNDARY != 0 {
                        Action::End(TINFLStatus::BlockBoundary)
                    } else {
                        Action::Jump(ReadBlockHeader)
                    }
                    #[cfg(not(feature = "block-boundary"))]
                    {
                        Action::Jump(ReadBlockHeader)
                    }
                }
            }),

            ReadAdler32 => generate_state!(state, 'state_machine, {
                if l.counter < 4 {
                    if l.num_bits != 0 {
                        read_bits(&mut l, 8, &mut in_iter, flags, |l, bits| {
                            r.z_adler32 <<= 8;
                            r.z_adler32 |= bits as u32;
                            l.counter += 1;
                            Action::None
                        })
                    } else {
                        read_byte(&mut in_iter, flags, |byte| {
                            r.z_adler32 <<= 8;
                            r.z_adler32 |= u32::from(byte);
                            l.counter += 1;
                            Action::None
                        })
                    }
                } else {
                    Action::Jump(DoneForever)
                }
            }),

            // We are done.
            DoneForever => break TINFLStatus::Done,

            // Anything else indicates failure.
            // BadZlibHeader | BadRawLength | BadDistOrLiteralTableLength | BlockTypeUnexpected |
            // DistanceOutOfBounds |
            // BadTotalSymbols | BadCodeSizeDistPrevLookup | BadCodeSizeSum | InvalidLitlen |
            // InvalidDist | InvalidCodeLen
            _ => break TINFLStatus::Failed,
        };
    };

    let in_undo = if status != TINFLStatus::NeedsMoreInput
        && status != TINFLStatus::FailedCannotMakeProgress
    {
        undo_bytes(&mut l, (in_buf.len() - in_iter.bytes_left()) as u32) as usize
    } else {
        0
    };

    // If we're returning after completing a block, prepare for the next block when called again.
    #[cfg(feature = "block-boundary")]
    if status == TINFLStatus::BlockBoundary {
        state = State::ReadBlockHeader;
    }

    // Make sure HasMoreOutput overrides NeedsMoreInput if the output buffer is full.
    // (Unless the missing input is the adler32 value in which case we don't need to write anything.)
    // TODO: May want to see if we can do this in a better way.
    if status == TINFLStatus::NeedsMoreInput
        && out_buf.bytes_left() == 0
        && state != State::ReadAdler32
    {
        status = TINFLStatus::HasMoreOutput
    }

    r.state = state;
    r.bit_buf = l.bit_buf;
    r.num_bits = l.num_bits;
    r.dist = l.dist;
    r.counter = l.counter;
    r.num_extra = l.num_extra;

    r.bit_buf &= ((1 as BitBuffer) << r.num_bits) - 1;

    // If this is a zlib stream, and update the adler32 checksum with the decompressed bytes if
    // requested.
    let need_adler = if (flags & TINFL_FLAG_IGNORE_ADLER32) == 0 {
        flags & (TINFL_FLAG_PARSE_ZLIB_HEADER | TINFL_FLAG_COMPUTE_ADLER32) != 0
    } else {
        // If TINFL_FLAG_IGNORE_ADLER32 is enabled, ignore the checksum.
        false
    };
    if need_adler && status as i32 >= 0 {
        let out_buf_pos = out_buf.position();
        r.check_adler32 = update_adler32(r.check_adler32, &out_buf.get_ref()[out_pos..out_buf_pos]);

        // disabled so that random input from fuzzer would not be rejected early,
        // before it has a chance to reach interesting parts of code
        if !cfg!(fuzzing) {
            // Once we are done, check if the checksum matches with the one provided in the zlib header.
            if status == TINFLStatus::Done
                && flags & TINFL_FLAG_PARSE_ZLIB_HEADER != 0
                && r.check_adler32 != r.z_adler32
            {
                status = TINFLStatus::Adler32Mismatch;
            }
        }
    }

    (
        status,
        in_buf.len() - in_iter.bytes_left() - in_undo,
        out_buf.position() - out_pos,
    )
}

#[cfg(test)]
mod test {
    use super::*;

    //TODO: Fix these.

    fn tinfl_decompress_oxide<'i>(
        r: &mut DecompressorOxide,
        input_buffer: &'i [u8],
        output_buffer: &mut [u8],
        flags: u32,
    ) -> (TINFLStatus, &'i [u8], usize) {
        let (status, in_pos, out_pos) = decompress(r, input_buffer, output_buffer, 0, flags);
        (status, &input_buffer[in_pos..], out_pos)
    }

    #[test]
    fn decompress_zlib() {
        let encoded = [
            120, 156, 243, 72, 205, 201, 201, 215, 81, 168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19,
        ];
        let flags = TINFL_FLAG_COMPUTE_ADLER32 | TINFL_FLAG_PARSE_ZLIB_HEADER;

        let mut b = DecompressorOxide::new();
        const LEN: usize = 32;
        let mut b_buf = [0; LEN];

        // This should fail with the out buffer being to small.
        let b_status = tinfl_decompress_oxide(&mut b, &encoded[..], &mut b_buf, flags);

        assert!(b_status.0 == TINFLStatus::Failed);

        let flags = flags | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

        b = DecompressorOxide::new();

        // With TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF set this should no longer fail.
        let b_status = tinfl_decompress_oxide(&mut b, &encoded[..], &mut b_buf, flags);

        assert_eq!(b_buf[..b_status.2], b"Hello, zlib!"[..]);
        assert!(b_status.0 == TINFLStatus::Done);
    }

    #[cfg(feature = "with-alloc")]
    #[test]
    fn raw_block() {
        const LEN: usize = 64;

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

        //let flags = TINFL_FLAG_COMPUTE_ADLER32 | TINFL_FLAG_PARSE_ZLIB_HEADER |
        let flags = TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

        let mut b = DecompressorOxide::new();

        let mut b_buf = [0; LEN];

        let b_status = tinfl_decompress_oxide(&mut b, &encoded[..], &mut b_buf, flags);
        assert_eq!(b_buf[..b_status.2], text[..]);
        assert_eq!(b_status.0, TINFLStatus::Done);
    }

    fn masked_lookup(table: &HuffmanTable, bit_buf: BitBuffer) -> (i32, u32) {
        let ret = table.lookup(bit_buf);
        (ret.0 & 511, ret.1)
    }

    #[test]
    fn fixed_table_lookup() {
        let mut d = DecompressorOxide::new();
        d.block_type = 1;
        start_static_table(&mut d);
        let mut l = LocalVars {
            bit_buf: d.bit_buf,
            num_bits: d.num_bits,
            dist: d.dist,
            counter: d.counter,
            num_extra: d.num_extra,
        };
        init_tree(&mut d, &mut l).unwrap();
        let llt = &d.tables[LITLEN_TABLE];
        let dt = &d.tables[DIST_TABLE];
        assert_eq!(masked_lookup(llt, 0b00001100), (0, 8));
        assert_eq!(masked_lookup(llt, 0b00011110), (72, 8));
        assert_eq!(masked_lookup(llt, 0b01011110), (74, 8));
        assert_eq!(masked_lookup(llt, 0b11111101), (143, 8));
        assert_eq!(masked_lookup(llt, 0b000010011), (144, 9));
        assert_eq!(masked_lookup(llt, 0b111111111), (255, 9));
        assert_eq!(masked_lookup(llt, 0b00000000), (256, 7));
        assert_eq!(masked_lookup(llt, 0b1110100), (279, 7));
        assert_eq!(masked_lookup(llt, 0b00000011), (280, 8));
        assert_eq!(masked_lookup(llt, 0b11100011), (287, 8));

        assert_eq!(masked_lookup(dt, 0), (0, 5));
        assert_eq!(masked_lookup(dt, 20), (5, 5));
    }

    // Only run this test with alloc enabled as it uses a larger buffer.
    #[cfg(feature = "with-alloc")]
    fn check_result(input: &[u8], expected_status: TINFLStatus, expected_state: State, zlib: bool) {
        let mut r = DecompressorOxide::default();
        let mut output_buf = vec![0; 1024 * 32];
        let flags = if zlib {
            inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
        } else {
            0
        } | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF
            | TINFL_FLAG_HAS_MORE_INPUT;
        let (d_status, _in_bytes, _out_bytes) =
            decompress(&mut r, input, &mut output_buf, 0, flags);
        assert_eq!(expected_status, d_status);
        assert_eq!(expected_state, r.state);
    }

    #[cfg(feature = "with-alloc")]
    #[test]
    fn bogus_input() {
        use self::check_result as cr;
        const F: TINFLStatus = TINFLStatus::Failed;
        const OK: TINFLStatus = TINFLStatus::Done;
        // Bad CM.
        cr(&[0x77, 0x85], F, State::BadZlibHeader, true);
        // Bad window size (but check is correct).
        cr(&[0x88, 0x98], F, State::BadZlibHeader, true);
        // Bad check bits.
        cr(&[0x78, 0x98], F, State::BadZlibHeader, true);

        // Too many code lengths. (From inflate library issues)
        cr(
            b"M\xff\xffM*\xad\xad\xad\xad\xad\xad\xad\xcd\xcd\xcdM",
            F,
            State::BadDistOrLiteralTableLength,
            false,
        );

        // Bad CLEN (also from inflate library issues)
        cr(
            b"\xdd\xff\xff*M\x94ffffffffff",
            F,
            State::BadDistOrLiteralTableLength,
            false,
        );

        // Port of inflate coverage tests from zlib-ng
        // https://github.com/Dead2/zlib-ng/blob/develop/test/infcover.c
        let c = |a, b, c| cr(a, b, c, false);

        // Invalid uncompressed/raw block length.
        c(&[0, 0, 0, 0, 0], F, State::BadRawLength);
        // Ok empty uncompressed block.
        c(&[3, 0], OK, State::DoneForever);
        // Invalid block type.
        c(&[6], F, State::BlockTypeUnexpected);
        // Ok uncompressed block.
        c(&[1, 1, 0, 0xfe, 0xff, 0], OK, State::DoneForever);
        // Too many litlens, we handle this later than zlib, so this test won't
        // give the same result.
        //        c(&[0xfc, 0, 0], F, State::BadTotalSymbols);
        // Invalid set of code lengths - TODO Check if this is the correct error for this.
        c(&[4, 0, 0xfe, 0xff], F, State::BadTotalSymbols);
        // Invalid repeat in list of code lengths.
        // (Try to repeat a non-existent code.)
        c(&[4, 0, 0x24, 0x49, 0], F, State::BadCodeSizeDistPrevLookup);
        // Missing end of block code (should we have a separate error for this?) - fails on further input
        //    c(&[4, 0, 0x24, 0xe9, 0xff, 0x6d], F, State::BadTotalSymbols);
        // Invalid set of literals/lengths
        c(
            &[
                4, 0x80, 0x49, 0x92, 0x24, 0x49, 0x92, 0x24, 0x71, 0xff, 0xff, 0x93, 0x11, 0,
            ],
            F,
            State::BadTotalSymbols,
        );
        // Invalid set of distances _ needsmoreinput
        // c(&[4, 0x80, 0x49, 0x92, 0x24, 0x49, 0x92, 0x24, 0x0f, 0xb4, 0xff, 0xff, 0xc3, 0x84], F, State::BadTotalSymbols);
        // Invalid distance code
        c(&[2, 0x7e, 0xff, 0xff], F, State::InvalidDist);

        // Distance refers to position before the start
        c(
            &[0x0c, 0xc0, 0x81, 0, 0, 0, 0, 0, 0x90, 0xff, 0x6b, 0x4, 0],
            F,
            State::DistanceOutOfBounds,
        );

        // Trailer
        // Bad gzip trailer checksum GZip header not handled by miniz_oxide
        //cr(&[0x1f, 0x8b, 0x08 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0x03, 0, 0, 0, 0, 0x01], F, State::BadCRC, false)
        // Bad gzip trailer length
        //cr(&[0x1f, 0x8b, 0x08 ,0 ,0 ,0 ,0 ,0 ,0 ,0 ,0x03, 0, 0, 0, 0, 0, 0, 0, 0, 0x01], F, State::BadCRC, false)
    }

    #[test]
    fn empty_output_buffer_non_wrapping() {
        let encoded = [
            120, 156, 243, 72, 205, 201, 201, 215, 81, 168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19,
        ];
        let flags = TINFL_FLAG_COMPUTE_ADLER32
            | TINFL_FLAG_PARSE_ZLIB_HEADER
            | TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
        let mut r = DecompressorOxide::new();
        let mut output_buf: [u8; 0] = [];
        // Check that we handle an empty buffer properly and not panicking.
        // https://github.com/Frommi/miniz_oxide/issues/23
        let res = decompress(&mut r, &encoded, &mut output_buf, 0, flags);
        assert!(res == (TINFLStatus::HasMoreOutput, 4, 0));
    }

    #[test]
    fn empty_output_buffer_wrapping() {
        let encoded = [
            0x73, 0x49, 0x4d, 0xcb, 0x49, 0x2c, 0x49, 0x55, 0x00, 0x11, 0x00,
        ];
        let flags = TINFL_FLAG_COMPUTE_ADLER32;
        let mut r = DecompressorOxide::new();
        let mut output_buf: [u8; 0] = [];
        // Check that we handle an empty buffer properly and not panicking.
        // https://github.com/Frommi/miniz_oxide/issues/23
        let res = decompress(&mut r, &encoded, &mut output_buf, 0, flags);
        assert!(res == (TINFLStatus::HasMoreOutput, 2, 0));
    }

    #[test]
    fn dist_extra_bits() {
        use self::num_extra_bits_for_distance_code;
        // Number of extra bits for each distance code.
        const DIST_EXTRA: [u8; 29] = [
            0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12,
            12, 13,
        ];

        for (i, &dist) in DIST_EXTRA.iter().enumerate() {
            assert_eq!(dist, num_extra_bits_for_distance_code(i as u8));
        }
    }

    #[test]
    fn check_tree() {
        let mut r = DecompressorOxide::new();
        let mut l = LocalVars {
            bit_buf: 0,
            num_bits: 0,
            dist: 0,
            counter: 0,
            num_extra: 0,
        };

        r.code_size_huffman[0] = 1;
        r.code_size_huffman[1] = 1;
        //r.code_size_huffman[2] = 3;
        //r.code_size_huffman[3] = 3;
        //r.code_size_huffman[1] = 4;
        r.block_type = HUFFLEN_TABLE as u8;
        r.table_sizes[HUFFLEN_TABLE] = 4;
        let res = init_tree(&mut r, &mut l).unwrap();

        let status = match res {
            Action::Jump(s) => s,
            _ => {
                //println!("issue");
                return;
            }
        };
        //println!("status {:?}", status);
        assert!(status != BadTotalSymbols);
    }

    #[test]
    fn reverse_bits_lookup() {
        use super::reverse_bits;
        for i in 0..512 {
            assert_eq!(reverse_bits(i), i.reverse_bits());
        }
    }
}
