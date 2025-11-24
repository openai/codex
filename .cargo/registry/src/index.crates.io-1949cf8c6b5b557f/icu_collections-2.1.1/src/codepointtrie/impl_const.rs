// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

pub const FAST_TYPE_SHIFT: i32 = 6;

/// Number of entries in a data block for code points below the fast limit. 64=0x40
pub const FAST_TYPE_DATA_BLOCK_LENGTH: u32 = 1 << FAST_TYPE_SHIFT;

/// Mask for getting the lower bits for the in-fast-data-block offset.
pub const FAST_TYPE_DATA_MASK: u32 = FAST_TYPE_DATA_BLOCK_LENGTH - 1;

/// Fast indexing limit for "fast"-type trie
pub const FAST_TYPE_FAST_INDEXING_MAX: u32 = 0xffff;

/// Fast indexing limit for "small"-type trie
pub const SMALL_TYPE_FAST_INDEXING_MAX: u32 = 0xfff;

/// Offset from dataLength (to be subtracted) for fetching the
/// value returned for out-of-range code points and ill-formed UTF-8/16.
pub const ERROR_VALUE_NEG_DATA_OFFSET: u32 = 1;

/// Offset from dataLength (to be subtracted) for fetching the
/// value returned for code points highStart..U+10FFFF.
pub const HIGH_VALUE_NEG_DATA_OFFSET: u32 = 2;

/// The length of the BMP index table. 1024=0x400
pub const BMP_INDEX_LENGTH: u32 = 0x10000 >> FAST_TYPE_SHIFT;

pub const SMALL_LIMIT: u32 = 0x1000;

pub const SMALL_INDEX_LENGTH: u32 = SMALL_LIMIT >> FAST_TYPE_SHIFT;

/// Shift size for getting the index-3 table offset.
pub const SHIFT_3: u32 = 4;

/// Shift size for getting the index-2 table offset.
pub const SHIFT_2: u32 = 5 + SHIFT_3;

/// Shift size for getting the index-1 table offset.
pub const SHIFT_1: u32 = 5 + SHIFT_2;

/// Difference between two shift sizes,
///  for getting an index-2 offset from an index-3 offset. 5=9-4
pub const SHIFT_2_3: u32 = SHIFT_2 - SHIFT_3;

/// Difference between two shift sizes,
/// for getting an index-1 offset from an index-2 offset. 5=14-9
pub const SHIFT_1_2: u32 = SHIFT_1 - SHIFT_2;

/// Number of index-1 entries for the BMP. (4)
/// This part of the index-1 table is omitted from the serialized form.
pub const OMITTED_BMP_INDEX_1_LENGTH: u32 = 0x10000 >> SHIFT_1;

/// Number of entries in an index-2 block. 32=0x20
pub const INDEX_2_BLOCK_LENGTH: u32 = 1 << SHIFT_1_2;

/// Mask for getting the lower bits for the in-index-2-block offset.
pub const INDEX_2_MASK: u32 = INDEX_2_BLOCK_LENGTH - 1;

/// Number of code points per index-2 table entry. 512=0x200
pub const CP_PER_INDEX_2_ENTRY: u32 = 1 << SHIFT_2;

/// Number of entries in an index-3 block. 32=0x20
pub const INDEX_3_BLOCK_LENGTH: u32 = 1 << SHIFT_2_3;

/// Mask for getting the lower bits for the in-index-3-block offset.
pub const INDEX_3_MASK: u32 = INDEX_3_BLOCK_LENGTH - 1;

/// Number of entries in a small data block. 16=0x10
pub const SMALL_DATA_BLOCK_LENGTH: u32 = 1 << SHIFT_3;

/// Mask for getting the lower bits for the in-small-data-block offset.
pub const SMALL_DATA_MASK: u32 = SMALL_DATA_BLOCK_LENGTH - 1;

pub const CODE_POINT_MAX: u32 = 0x10ffff;
