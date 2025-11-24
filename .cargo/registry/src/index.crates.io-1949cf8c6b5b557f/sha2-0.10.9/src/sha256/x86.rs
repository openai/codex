//! SHA-256 `x86`/`x86_64` backend

#![allow(clippy::many_single_char_names)]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

unsafe fn schedule(v0: __m128i, v1: __m128i, v2: __m128i, v3: __m128i) -> __m128i {
    let t1 = _mm_sha256msg1_epu32(v0, v1);
    let t2 = _mm_alignr_epi8(v3, v2, 4);
    let t3 = _mm_add_epi32(t1, t2);
    _mm_sha256msg2_epu32(t3, v3)
}

macro_rules! rounds4 {
    ($abef:ident, $cdgh:ident, $rest:expr, $i:expr) => {{
        let k = crate::consts::K32X4[$i];
        let kv = _mm_set_epi32(k[0] as i32, k[1] as i32, k[2] as i32, k[3] as i32);
        let t1 = _mm_add_epi32($rest, kv);
        $cdgh = _mm_sha256rnds2_epu32($cdgh, $abef, t1);
        let t2 = _mm_shuffle_epi32(t1, 0x0E);
        $abef = _mm_sha256rnds2_epu32($abef, $cdgh, t2);
    }};
}

macro_rules! schedule_rounds4 {
    (
        $abef:ident, $cdgh:ident,
        $w0:expr, $w1:expr, $w2:expr, $w3:expr, $w4:expr,
        $i: expr
    ) => {{
        $w4 = schedule($w0, $w1, $w2, $w3);
        rounds4!($abef, $cdgh, $w4, $i);
    }};
}

// we use unaligned loads with `__m128i` pointers
#[allow(clippy::cast_ptr_alignment)]
#[target_feature(enable = "sha,sse2,ssse3,sse4.1")]
unsafe fn digest_blocks(state: &mut [u32; 8], blocks: &[[u8; 64]]) {
    #[allow(non_snake_case)]
    let MASK: __m128i = _mm_set_epi64x(
        0x0C0D_0E0F_0809_0A0Bu64 as i64,
        0x0405_0607_0001_0203u64 as i64,
    );

    let state_ptr = state.as_ptr() as *const __m128i;
    let dcba = _mm_loadu_si128(state_ptr.add(0));
    let efgh = _mm_loadu_si128(state_ptr.add(1));

    let cdab = _mm_shuffle_epi32(dcba, 0xB1);
    let efgh = _mm_shuffle_epi32(efgh, 0x1B);
    let mut abef = _mm_alignr_epi8(cdab, efgh, 8);
    let mut cdgh = _mm_blend_epi16(efgh, cdab, 0xF0);

    for block in blocks {
        let abef_save = abef;
        let cdgh_save = cdgh;

        let data_ptr = block.as_ptr() as *const __m128i;
        let mut w0 = _mm_shuffle_epi8(_mm_loadu_si128(data_ptr.add(0)), MASK);
        let mut w1 = _mm_shuffle_epi8(_mm_loadu_si128(data_ptr.add(1)), MASK);
        let mut w2 = _mm_shuffle_epi8(_mm_loadu_si128(data_ptr.add(2)), MASK);
        let mut w3 = _mm_shuffle_epi8(_mm_loadu_si128(data_ptr.add(3)), MASK);
        let mut w4;

        rounds4!(abef, cdgh, w0, 0);
        rounds4!(abef, cdgh, w1, 1);
        rounds4!(abef, cdgh, w2, 2);
        rounds4!(abef, cdgh, w3, 3);
        schedule_rounds4!(abef, cdgh, w0, w1, w2, w3, w4, 4);
        schedule_rounds4!(abef, cdgh, w1, w2, w3, w4, w0, 5);
        schedule_rounds4!(abef, cdgh, w2, w3, w4, w0, w1, 6);
        schedule_rounds4!(abef, cdgh, w3, w4, w0, w1, w2, 7);
        schedule_rounds4!(abef, cdgh, w4, w0, w1, w2, w3, 8);
        schedule_rounds4!(abef, cdgh, w0, w1, w2, w3, w4, 9);
        schedule_rounds4!(abef, cdgh, w1, w2, w3, w4, w0, 10);
        schedule_rounds4!(abef, cdgh, w2, w3, w4, w0, w1, 11);
        schedule_rounds4!(abef, cdgh, w3, w4, w0, w1, w2, 12);
        schedule_rounds4!(abef, cdgh, w4, w0, w1, w2, w3, 13);
        schedule_rounds4!(abef, cdgh, w0, w1, w2, w3, w4, 14);
        schedule_rounds4!(abef, cdgh, w1, w2, w3, w4, w0, 15);

        abef = _mm_add_epi32(abef, abef_save);
        cdgh = _mm_add_epi32(cdgh, cdgh_save);
    }

    let feba = _mm_shuffle_epi32(abef, 0x1B);
    let dchg = _mm_shuffle_epi32(cdgh, 0xB1);
    let dcba = _mm_blend_epi16(feba, dchg, 0xF0);
    let hgef = _mm_alignr_epi8(dchg, feba, 8);

    let state_ptr_mut = state.as_mut_ptr() as *mut __m128i;
    _mm_storeu_si128(state_ptr_mut.add(0), dcba);
    _mm_storeu_si128(state_ptr_mut.add(1), hgef);
}

cpufeatures::new!(shani_cpuid, "sha", "sse2", "ssse3", "sse4.1");

pub fn compress(state: &mut [u32; 8], blocks: &[[u8; 64]]) {
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
