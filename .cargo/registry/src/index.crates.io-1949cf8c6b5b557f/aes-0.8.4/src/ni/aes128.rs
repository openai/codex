use super::{arch::*, utils::*};
use crate::{Block, Block8};
use cipher::inout::InOut;
use core::mem;

/// AES-128 round keys
pub(super) type RoundKeys = [__m128i; 11];

#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn encrypt1(keys: &RoundKeys, block: InOut<'_, '_, Block>) {
    let (in_ptr, out_ptr) = block.into_raw();
    let mut b = _mm_loadu_si128(in_ptr as *const __m128i);
    b = _mm_xor_si128(b, keys[0]);
    b = _mm_aesenc_si128(b, keys[1]);
    b = _mm_aesenc_si128(b, keys[2]);
    b = _mm_aesenc_si128(b, keys[3]);
    b = _mm_aesenc_si128(b, keys[4]);
    b = _mm_aesenc_si128(b, keys[5]);
    b = _mm_aesenc_si128(b, keys[6]);
    b = _mm_aesenc_si128(b, keys[7]);
    b = _mm_aesenc_si128(b, keys[8]);
    b = _mm_aesenc_si128(b, keys[9]);
    b = _mm_aesenclast_si128(b, keys[10]);
    _mm_storeu_si128(out_ptr as *mut __m128i, b);
}

#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn encrypt8(keys: &RoundKeys, blocks: InOut<'_, '_, Block8>) {
    let (in_ptr, out_ptr) = blocks.into_raw();
    let mut b = load8(in_ptr);
    xor8(&mut b, keys[0]);
    aesenc8(&mut b, keys[1]);
    aesenc8(&mut b, keys[2]);
    aesenc8(&mut b, keys[3]);
    aesenc8(&mut b, keys[4]);
    aesenc8(&mut b, keys[5]);
    aesenc8(&mut b, keys[6]);
    aesenc8(&mut b, keys[7]);
    aesenc8(&mut b, keys[8]);
    aesenc8(&mut b, keys[9]);
    aesenclast8(&mut b, keys[10]);
    store8(out_ptr, b);
}

#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn decrypt1(keys: &RoundKeys, block: InOut<'_, '_, Block>) {
    let (in_ptr, out_ptr) = block.into_raw();
    let mut b = _mm_loadu_si128(in_ptr as *const __m128i);
    b = _mm_xor_si128(b, keys[10]);
    b = _mm_aesdec_si128(b, keys[9]);
    b = _mm_aesdec_si128(b, keys[8]);
    b = _mm_aesdec_si128(b, keys[7]);
    b = _mm_aesdec_si128(b, keys[6]);
    b = _mm_aesdec_si128(b, keys[5]);
    b = _mm_aesdec_si128(b, keys[4]);
    b = _mm_aesdec_si128(b, keys[3]);
    b = _mm_aesdec_si128(b, keys[2]);
    b = _mm_aesdec_si128(b, keys[1]);
    b = _mm_aesdeclast_si128(b, keys[0]);
    _mm_storeu_si128(out_ptr as *mut __m128i, b);
}

#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn decrypt8(keys: &RoundKeys, blocks: InOut<'_, '_, Block8>) {
    let (in_ptr, out_ptr) = blocks.into_raw();
    let mut b = load8(in_ptr);
    xor8(&mut b, keys[10]);
    aesdec8(&mut b, keys[9]);
    aesdec8(&mut b, keys[8]);
    aesdec8(&mut b, keys[7]);
    aesdec8(&mut b, keys[6]);
    aesdec8(&mut b, keys[5]);
    aesdec8(&mut b, keys[4]);
    aesdec8(&mut b, keys[3]);
    aesdec8(&mut b, keys[2]);
    aesdec8(&mut b, keys[1]);
    aesdeclast8(&mut b, keys[0]);
    store8(out_ptr, b);
}

macro_rules! expand_round {
    ($keys:expr, $pos:expr, $round:expr) => {
        let mut t1 = $keys[$pos - 1];
        let mut t2;
        let mut t3;

        t2 = _mm_aeskeygenassist_si128(t1, $round);
        t2 = _mm_shuffle_epi32(t2, 0xff);
        t3 = _mm_slli_si128(t1, 0x4);
        t1 = _mm_xor_si128(t1, t3);
        t3 = _mm_slli_si128(t3, 0x4);
        t1 = _mm_xor_si128(t1, t3);
        t3 = _mm_slli_si128(t3, 0x4);
        t1 = _mm_xor_si128(t1, t3);
        t1 = _mm_xor_si128(t1, t2);

        $keys[$pos] = t1;
    };
}

#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn expand_key(key: &[u8; 16]) -> RoundKeys {
    // SAFETY: `RoundKeys` is a `[__m128i; 11]` which can be initialized
    // with all zeroes.
    let mut keys: RoundKeys = mem::zeroed();

    let k = _mm_loadu_si128(key.as_ptr() as *const __m128i);
    keys[0] = k;

    expand_round!(keys, 1, 0x01);
    expand_round!(keys, 2, 0x02);
    expand_round!(keys, 3, 0x04);
    expand_round!(keys, 4, 0x08);
    expand_round!(keys, 5, 0x10);
    expand_round!(keys, 6, 0x20);
    expand_round!(keys, 7, 0x40);
    expand_round!(keys, 8, 0x80);
    expand_round!(keys, 9, 0x1B);
    expand_round!(keys, 10, 0x36);

    keys
}

#[inline]
#[target_feature(enable = "aes")]
pub(super) unsafe fn inv_expanded_keys(keys: &RoundKeys) -> RoundKeys {
    [
        keys[0],
        _mm_aesimc_si128(keys[1]),
        _mm_aesimc_si128(keys[2]),
        _mm_aesimc_si128(keys[3]),
        _mm_aesimc_si128(keys[4]),
        _mm_aesimc_si128(keys[5]),
        _mm_aesimc_si128(keys[6]),
        _mm_aesimc_si128(keys[7]),
        _mm_aesimc_si128(keys[8]),
        _mm_aesimc_si128(keys[9]),
        keys[10],
    ]
}
