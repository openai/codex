//! AES key expansion support.

use core::{arch::aarch64::*, mem, slice};

// Stable "polyfills" for unstable core::arch::aarch64 intrinsics
// TODO(tarcieri): remove when these intrinsics have been stabilized
use super::intrinsics::{vaeseq_u8, vaesimcq_u8};

/// There are 4 AES words in a block.
const BLOCK_WORDS: usize = 4;

/// The AES (nee Rijndael) notion of a word is always 32-bits, or 4-bytes.
const WORD_SIZE: usize = 4;

/// AES round constants.
const ROUND_CONSTS: [u32; 10] = [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

/// AES key expansion.
#[target_feature(enable = "aes")]
pub unsafe fn expand_key<const L: usize, const N: usize>(key: &[u8; L]) -> [uint8x16_t; N] {
    assert!((L == 16 && N == 11) || (L == 24 && N == 13) || (L == 32 && N == 15));

    let mut expanded_keys: [uint8x16_t; N] = mem::zeroed();

    // Sanity check, as this is required in order for the following line to be sound.
    const _: () = assert!(mem::align_of::<uint8x16_t>() >= mem::align_of::<u32>());
    let keys_ptr: *mut u32 = expanded_keys.as_mut_ptr().cast();
    let columns = slice::from_raw_parts_mut(keys_ptr, N * BLOCK_WORDS);

    for (i, chunk) in key.chunks_exact(WORD_SIZE).enumerate() {
        columns[i] = u32::from_ne_bytes(chunk.try_into().unwrap());
    }

    // From "The Rijndael Block Cipher" Section 4.1:
    // > The number of columns of the Cipher Key is denoted by `Nk` and is
    // > equal to the key length divided by 32 [bits].
    let nk = L / WORD_SIZE;

    for i in nk..(N * BLOCK_WORDS) {
        let mut word = columns[i - 1];

        if i % nk == 0 {
            word = sub_word(word).rotate_right(8) ^ ROUND_CONSTS[i / nk - 1];
        } else if nk > 6 && i % nk == 4 {
            word = sub_word(word);
        }

        columns[i] = columns[i - nk] ^ word;
    }

    expanded_keys
}

/// Compute inverse expanded keys (for decryption).
///
/// This is the reverse of the encryption keys, with the Inverse Mix Columns
/// operation applied to all but the first and last expanded key.
#[target_feature(enable = "aes")]
pub(super) unsafe fn inv_expanded_keys<const N: usize>(expanded_keys: &mut [uint8x16_t; N]) {
    assert!(N == 11 || N == 13 || N == 15);

    for ek in expanded_keys.iter_mut().take(N - 1).skip(1) {
        *ek = vaesimcq_u8(*ek);
    }

    expanded_keys.reverse();
}

/// Sub bytes for a single AES word: used for key expansion.
#[inline]
#[target_feature(enable = "aes")]
unsafe fn sub_word(input: u32) -> u32 {
    let input = vreinterpretq_u8_u32(vdupq_n_u32(input));

    // AES single round encryption (with a "round" key of all zeros)
    let sub_input = vaeseq_u8(input, vdupq_n_u8(0));

    vgetq_lane_u32(vreinterpretq_u32_u8(sub_input), 0)
}
