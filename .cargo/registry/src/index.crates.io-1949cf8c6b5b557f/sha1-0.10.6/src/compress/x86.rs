//! SHA-1 `x86`/`x86_64` backend

#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

macro_rules! rounds4 {
    ($h0:ident, $h1:ident, $wk:expr, $i:expr) => {
        _mm_sha1rnds4_epu32($h0, _mm_sha1nexte_epu32($h1, $wk), $i)
    };
}

macro_rules! schedule {
    ($v0:expr, $v1:expr, $v2:expr, $v3:expr) => {
        _mm_sha1msg2_epu32(_mm_xor_si128(_mm_sha1msg1_epu32($v0, $v1), $v2), $v3)
    };
}

macro_rules! schedule_rounds4 {
    (
        $h0:ident, $h1:ident,
        $w0:expr, $w1:expr, $w2:expr, $w3:expr, $w4:expr,
        $i:expr
    ) => {
        $w4 = schedule!($w0, $w1, $w2, $w3);
        $h1 = rounds4!($h0, $h1, $w4, $i);
    };
}

#[target_feature(enable = "sha,sse2,ssse3,sse4.1")]
unsafe fn digest_blocks(state: &mut [u32; 5], blocks: &[[u8; 64]]) {
    #[allow(non_snake_case)]
    let MASK: __m128i = _mm_set_epi64x(0x0001_0203_0405_0607, 0x0809_0A0B_0C0D_0E0F);

    let mut state_abcd = _mm_set_epi32(
        state[0] as i32,
        state[1] as i32,
        state[2] as i32,
        state[3] as i32,
    );
    let mut state_e = _mm_set_epi32(state[4] as i32, 0, 0, 0);

    for block in blocks {
        // SAFETY: we use only unaligned loads with this pointer
        #[allow(clippy::cast_ptr_alignment)]
        let block_ptr = block.as_ptr() as *const __m128i;

        let mut w0 = _mm_shuffle_epi8(_mm_loadu_si128(block_ptr.offset(0)), MASK);
        let mut w1 = _mm_shuffle_epi8(_mm_loadu_si128(block_ptr.offset(1)), MASK);
        let mut w2 = _mm_shuffle_epi8(_mm_loadu_si128(block_ptr.offset(2)), MASK);
        let mut w3 = _mm_shuffle_epi8(_mm_loadu_si128(block_ptr.offset(3)), MASK);
        #[allow(clippy::needless_late_init)]
        let mut w4;

        let mut h0 = state_abcd;
        let mut h1 = _mm_add_epi32(state_e, w0);

        // Rounds 0..20
        h1 = _mm_sha1rnds4_epu32(h0, h1, 0);
        h0 = rounds4!(h1, h0, w1, 0);
        h1 = rounds4!(h0, h1, w2, 0);
        h0 = rounds4!(h1, h0, w3, 0);
        schedule_rounds4!(h0, h1, w0, w1, w2, w3, w4, 0);

        // Rounds 20..40
        schedule_rounds4!(h1, h0, w1, w2, w3, w4, w0, 1);
        schedule_rounds4!(h0, h1, w2, w3, w4, w0, w1, 1);
        schedule_rounds4!(h1, h0, w3, w4, w0, w1, w2, 1);
        schedule_rounds4!(h0, h1, w4, w0, w1, w2, w3, 1);
        schedule_rounds4!(h1, h0, w0, w1, w2, w3, w4, 1);

        // Rounds 40..60
        schedule_rounds4!(h0, h1, w1, w2, w3, w4, w0, 2);
        schedule_rounds4!(h1, h0, w2, w3, w4, w0, w1, 2);
        schedule_rounds4!(h0, h1, w3, w4, w0, w1, w2, 2);
        schedule_rounds4!(h1, h0, w4, w0, w1, w2, w3, 2);
        schedule_rounds4!(h0, h1, w0, w1, w2, w3, w4, 2);

        // Rounds 60..80
        schedule_rounds4!(h1, h0, w1, w2, w3, w4, w0, 3);
        schedule_rounds4!(h0, h1, w2, w3, w4, w0, w1, 3);
        schedule_rounds4!(h1, h0, w3, w4, w0, w1, w2, 3);
        schedule_rounds4!(h0, h1, w4, w0, w1, w2, w3, 3);
        schedule_rounds4!(h1, h0, w0, w1, w2, w3, w4, 3);

        state_abcd = _mm_add_epi32(state_abcd, h0);
        state_e = _mm_sha1nexte_epu32(h1, state_e);
    }

    state[0] = _mm_extract_epi32(state_abcd, 3) as u32;
    state[1] = _mm_extract_epi32(state_abcd, 2) as u32;
    state[2] = _mm_extract_epi32(state_abcd, 1) as u32;
    state[3] = _mm_extract_epi32(state_abcd, 0) as u32;
    state[4] = _mm_extract_epi32(state_e, 3) as u32;
}

cpufeatures::new!(shani_cpuid, "sha", "sse2", "ssse3", "sse4.1");

pub fn compress(state: &mut [u32; 5], blocks: &[[u8; 64]]) {
    // TODO: Replace with https://github.com/rust-lang/rfcs/pull/2725
    // after stabilization
    if shani_cpuid::get() {
        unsafe {
            digest_blocks(state, blocks);
        }
    } else {
        super::soft::compress(state, blocks);
    }
}
