// Implementation adapted from mbedtls.

use core::arch::{aarch64::*, asm};

use crate::consts::K64;

cpufeatures::new!(sha3_hwcap, "sha3");

pub fn compress(state: &mut [u64; 8], blocks: &[[u8; 128]]) {
    // TODO: Replace with https://github.com/rust-lang/rfcs/pull/2725
    // after stabilization
    if sha3_hwcap::get() {
        unsafe { sha512_compress(state, blocks) }
    } else {
        super::soft::compress(state, blocks);
    }
}

#[target_feature(enable = "sha3")]
unsafe fn sha512_compress(state: &mut [u64; 8], blocks: &[[u8; 128]]) {
    // SAFETY: Requires the sha3 feature.

    // Load state into vectors.
    let mut ab = vld1q_u64(state[0..2].as_ptr());
    let mut cd = vld1q_u64(state[2..4].as_ptr());
    let mut ef = vld1q_u64(state[4..6].as_ptr());
    let mut gh = vld1q_u64(state[6..8].as_ptr());

    // Iterate through the message blocks.
    for block in blocks {
        // Keep original state values.
        let ab_orig = ab;
        let cd_orig = cd;
        let ef_orig = ef;
        let gh_orig = gh;

        // Load the message block into vectors, assuming little endianness.
        let mut s0 = vreinterpretq_u64_u8(vrev64q_u8(vld1q_u8(block[0..16].as_ptr())));
        let mut s1 = vreinterpretq_u64_u8(vrev64q_u8(vld1q_u8(block[16..32].as_ptr())));
        let mut s2 = vreinterpretq_u64_u8(vrev64q_u8(vld1q_u8(block[32..48].as_ptr())));
        let mut s3 = vreinterpretq_u64_u8(vrev64q_u8(vld1q_u8(block[48..64].as_ptr())));
        let mut s4 = vreinterpretq_u64_u8(vrev64q_u8(vld1q_u8(block[64..80].as_ptr())));
        let mut s5 = vreinterpretq_u64_u8(vrev64q_u8(vld1q_u8(block[80..96].as_ptr())));
        let mut s6 = vreinterpretq_u64_u8(vrev64q_u8(vld1q_u8(block[96..112].as_ptr())));
        let mut s7 = vreinterpretq_u64_u8(vrev64q_u8(vld1q_u8(block[112..128].as_ptr())));

        // Rounds 0 and 1
        let mut initial_sum = vaddq_u64(s0, vld1q_u64(&K64[0]));
        let mut sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), gh);
        let mut intermed = vsha512hq_u64(sum, vextq_u64(ef, gh, 1), vextq_u64(cd, ef, 1));
        gh = vsha512h2q_u64(intermed, cd, ab);
        cd = vaddq_u64(cd, intermed);

        // Rounds 2 and 3
        initial_sum = vaddq_u64(s1, vld1q_u64(&K64[2]));
        sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), ef);
        intermed = vsha512hq_u64(sum, vextq_u64(cd, ef, 1), vextq_u64(ab, cd, 1));
        ef = vsha512h2q_u64(intermed, ab, gh);
        ab = vaddq_u64(ab, intermed);

        // Rounds 4 and 5
        initial_sum = vaddq_u64(s2, vld1q_u64(&K64[4]));
        sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), cd);
        intermed = vsha512hq_u64(sum, vextq_u64(ab, cd, 1), vextq_u64(gh, ab, 1));
        cd = vsha512h2q_u64(intermed, gh, ef);
        gh = vaddq_u64(gh, intermed);

        // Rounds 6 and 7
        initial_sum = vaddq_u64(s3, vld1q_u64(&K64[6]));
        sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), ab);
        intermed = vsha512hq_u64(sum, vextq_u64(gh, ab, 1), vextq_u64(ef, gh, 1));
        ab = vsha512h2q_u64(intermed, ef, cd);
        ef = vaddq_u64(ef, intermed);

        // Rounds 8 and 9
        initial_sum = vaddq_u64(s4, vld1q_u64(&K64[8]));
        sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), gh);
        intermed = vsha512hq_u64(sum, vextq_u64(ef, gh, 1), vextq_u64(cd, ef, 1));
        gh = vsha512h2q_u64(intermed, cd, ab);
        cd = vaddq_u64(cd, intermed);

        // Rounds 10 and 11
        initial_sum = vaddq_u64(s5, vld1q_u64(&K64[10]));
        sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), ef);
        intermed = vsha512hq_u64(sum, vextq_u64(cd, ef, 1), vextq_u64(ab, cd, 1));
        ef = vsha512h2q_u64(intermed, ab, gh);
        ab = vaddq_u64(ab, intermed);

        // Rounds 12 and 13
        initial_sum = vaddq_u64(s6, vld1q_u64(&K64[12]));
        sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), cd);
        intermed = vsha512hq_u64(sum, vextq_u64(ab, cd, 1), vextq_u64(gh, ab, 1));
        cd = vsha512h2q_u64(intermed, gh, ef);
        gh = vaddq_u64(gh, intermed);

        // Rounds 14 and 15
        initial_sum = vaddq_u64(s7, vld1q_u64(&K64[14]));
        sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), ab);
        intermed = vsha512hq_u64(sum, vextq_u64(gh, ab, 1), vextq_u64(ef, gh, 1));
        ab = vsha512h2q_u64(intermed, ef, cd);
        ef = vaddq_u64(ef, intermed);

        for t in (16..80).step_by(16) {
            // Rounds t and t + 1
            s0 = vsha512su1q_u64(vsha512su0q_u64(s0, s1), s7, vextq_u64(s4, s5, 1));
            initial_sum = vaddq_u64(s0, vld1q_u64(&K64[t]));
            sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), gh);
            intermed = vsha512hq_u64(sum, vextq_u64(ef, gh, 1), vextq_u64(cd, ef, 1));
            gh = vsha512h2q_u64(intermed, cd, ab);
            cd = vaddq_u64(cd, intermed);

            // Rounds t + 2 and t + 3
            s1 = vsha512su1q_u64(vsha512su0q_u64(s1, s2), s0, vextq_u64(s5, s6, 1));
            initial_sum = vaddq_u64(s1, vld1q_u64(&K64[t + 2]));
            sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), ef);
            intermed = vsha512hq_u64(sum, vextq_u64(cd, ef, 1), vextq_u64(ab, cd, 1));
            ef = vsha512h2q_u64(intermed, ab, gh);
            ab = vaddq_u64(ab, intermed);

            // Rounds t + 4 and t + 5
            s2 = vsha512su1q_u64(vsha512su0q_u64(s2, s3), s1, vextq_u64(s6, s7, 1));
            initial_sum = vaddq_u64(s2, vld1q_u64(&K64[t + 4]));
            sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), cd);
            intermed = vsha512hq_u64(sum, vextq_u64(ab, cd, 1), vextq_u64(gh, ab, 1));
            cd = vsha512h2q_u64(intermed, gh, ef);
            gh = vaddq_u64(gh, intermed);

            // Rounds t + 6 and t + 7
            s3 = vsha512su1q_u64(vsha512su0q_u64(s3, s4), s2, vextq_u64(s7, s0, 1));
            initial_sum = vaddq_u64(s3, vld1q_u64(&K64[t + 6]));
            sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), ab);
            intermed = vsha512hq_u64(sum, vextq_u64(gh, ab, 1), vextq_u64(ef, gh, 1));
            ab = vsha512h2q_u64(intermed, ef, cd);
            ef = vaddq_u64(ef, intermed);

            // Rounds t + 8 and t + 9
            s4 = vsha512su1q_u64(vsha512su0q_u64(s4, s5), s3, vextq_u64(s0, s1, 1));
            initial_sum = vaddq_u64(s4, vld1q_u64(&K64[t + 8]));
            sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), gh);
            intermed = vsha512hq_u64(sum, vextq_u64(ef, gh, 1), vextq_u64(cd, ef, 1));
            gh = vsha512h2q_u64(intermed, cd, ab);
            cd = vaddq_u64(cd, intermed);

            // Rounds t + 10 and t + 11
            s5 = vsha512su1q_u64(vsha512su0q_u64(s5, s6), s4, vextq_u64(s1, s2, 1));
            initial_sum = vaddq_u64(s5, vld1q_u64(&K64[t + 10]));
            sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), ef);
            intermed = vsha512hq_u64(sum, vextq_u64(cd, ef, 1), vextq_u64(ab, cd, 1));
            ef = vsha512h2q_u64(intermed, ab, gh);
            ab = vaddq_u64(ab, intermed);

            // Rounds t + 12 and t + 13
            s6 = vsha512su1q_u64(vsha512su0q_u64(s6, s7), s5, vextq_u64(s2, s3, 1));
            initial_sum = vaddq_u64(s6, vld1q_u64(&K64[t + 12]));
            sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), cd);
            intermed = vsha512hq_u64(sum, vextq_u64(ab, cd, 1), vextq_u64(gh, ab, 1));
            cd = vsha512h2q_u64(intermed, gh, ef);
            gh = vaddq_u64(gh, intermed);

            // Rounds t + 14 and t + 15
            s7 = vsha512su1q_u64(vsha512su0q_u64(s7, s0), s6, vextq_u64(s3, s4, 1));
            initial_sum = vaddq_u64(s7, vld1q_u64(&K64[t + 14]));
            sum = vaddq_u64(vextq_u64(initial_sum, initial_sum, 1), ab);
            intermed = vsha512hq_u64(sum, vextq_u64(gh, ab, 1), vextq_u64(ef, gh, 1));
            ab = vsha512h2q_u64(intermed, ef, cd);
            ef = vaddq_u64(ef, intermed);
        }

        // Add the block-specific state to the original state.
        ab = vaddq_u64(ab, ab_orig);
        cd = vaddq_u64(cd, cd_orig);
        ef = vaddq_u64(ef, ef_orig);
        gh = vaddq_u64(gh, gh_orig);
    }

    // Store vectors into state.
    vst1q_u64(state[0..2].as_mut_ptr(), ab);
    vst1q_u64(state[2..4].as_mut_ptr(), cd);
    vst1q_u64(state[4..6].as_mut_ptr(), ef);
    vst1q_u64(state[6..8].as_mut_ptr(), gh);
}

// TODO remove these polyfills once SHA3 intrinsics land

#[inline(always)]
unsafe fn vsha512hq_u64(
    mut hash_ed: uint64x2_t,
    hash_gf: uint64x2_t,
    kwh_kwh2: uint64x2_t,
) -> uint64x2_t {
    asm!(
        "SHA512H {:q}, {:q}, {:v}.2D",
        inout(vreg) hash_ed, in(vreg) hash_gf, in(vreg) kwh_kwh2,
        options(pure, nomem, nostack, preserves_flags)
    );
    hash_ed
}

#[inline(always)]
unsafe fn vsha512h2q_u64(
    mut sum_ab: uint64x2_t,
    hash_c_: uint64x2_t,
    hash_ab: uint64x2_t,
) -> uint64x2_t {
    asm!(
        "SHA512H2 {:q}, {:q}, {:v}.2D",
        inout(vreg) sum_ab, in(vreg) hash_c_, in(vreg) hash_ab,
        options(pure, nomem, nostack, preserves_flags)
    );
    sum_ab
}

#[inline(always)]
unsafe fn vsha512su0q_u64(mut w0_1: uint64x2_t, w2_: uint64x2_t) -> uint64x2_t {
    asm!(
        "SHA512SU0 {:v}.2D, {:v}.2D",
        inout(vreg) w0_1, in(vreg) w2_,
        options(pure, nomem, nostack, preserves_flags)
    );
    w0_1
}

#[inline(always)]
unsafe fn vsha512su1q_u64(
    mut s01_s02: uint64x2_t,
    w14_15: uint64x2_t,
    w9_10: uint64x2_t,
) -> uint64x2_t {
    asm!(
        "SHA512SU1 {:v}.2D, {:v}.2D, {:v}.2D",
        inout(vreg) s01_s02, in(vreg) w14_15, in(vreg) w9_10,
        options(pure, nomem, nostack, preserves_flags)
    );
    s01_s02
}
