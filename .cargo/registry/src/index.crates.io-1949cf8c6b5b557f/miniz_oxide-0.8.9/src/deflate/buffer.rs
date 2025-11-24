//! Buffer wrappers implementing default so we can allocate the buffers with `Box::default()`
//! to avoid stack copies. Box::new() doesn't at the moment, and using a vec means we would lose
//! static length info.

use crate::deflate::core::{LZ_DICT_SIZE, MAX_MATCH_LEN};
use alloc::boxed::Box;
use alloc::vec;

/// Size of the buffer of lz77 encoded data.
pub const LZ_CODE_BUF_SIZE: usize = 64 * 1024;
pub const LZ_CODE_BUF_MASK: usize = LZ_CODE_BUF_SIZE - 1;
/// Size of the output buffer.
pub const OUT_BUF_SIZE: usize = (LZ_CODE_BUF_SIZE * 13) / 10;
pub const LZ_DICT_FULL_SIZE: usize = LZ_DICT_SIZE + MAX_MATCH_LEN - 1 + 1;

/// Size of hash values in the hash chains.
pub const LZ_HASH_BITS: i32 = 15;
/// How many bits to shift when updating the current hash value.
pub const LZ_HASH_SHIFT: i32 = (LZ_HASH_BITS + 2) / 3;
/// Size of the chained hash tables.
pub const LZ_HASH_SIZE: usize = 1 << LZ_HASH_BITS;

#[inline]
pub const fn update_hash(current_hash: u16, byte: u8) -> u16 {
    ((current_hash << LZ_HASH_SHIFT) ^ byte as u16) & (LZ_HASH_SIZE as u16 - 1)
}

pub struct HashBuffers {
    pub dict: Box<[u8; LZ_DICT_FULL_SIZE]>,
    pub next: Box<[u16; LZ_DICT_SIZE]>,
    pub hash: Box<[u16; LZ_DICT_SIZE]>,
}

impl HashBuffers {
    #[inline]
    pub fn reset(&mut self) {
        self.dict.fill(0);
        self.next.fill(0);
        self.hash.fill(0);
    }
}

impl Default for HashBuffers {
    fn default() -> HashBuffers {
        HashBuffers {
            dict: vec![0; LZ_DICT_FULL_SIZE]
                .into_boxed_slice()
                .try_into()
                .unwrap(),
            next: vec![0; LZ_DICT_SIZE].into_boxed_slice().try_into().unwrap(),
            hash: vec![0; LZ_DICT_SIZE].into_boxed_slice().try_into().unwrap(),
        }
    }
}

pub struct LocalBuf {
    pub b: [u8; OUT_BUF_SIZE],
}

impl Default for LocalBuf {
    fn default() -> LocalBuf {
        LocalBuf {
            b: [0; OUT_BUF_SIZE],
        }
    }
}
