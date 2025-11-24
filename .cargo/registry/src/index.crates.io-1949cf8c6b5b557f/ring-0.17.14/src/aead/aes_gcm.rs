// Copyright 2015-2025 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use super::{
    aes::{self, Counter, Overlapping, OverlappingPartialBlock, BLOCK_LEN, ZERO_BLOCK},
    gcm,
    overlapping::IndexError,
    Aad, Nonce, Tag,
};
use crate::{
    cpu,
    error::{self, InputTooLongError},
    polyfill::{slice, sliceutil::overwrite_at_start, usize_from_u64_saturated},
};
use core::ops::RangeFrom;

#[cfg(any(
    all(target_arch = "aarch64", target_endian = "little"),
    all(target_arch = "arm", target_endian = "little"),
    target_arch = "x86",
    target_arch = "x86_64"
))]
use cpu::GetFeature as _;

mod aarch64;
mod aeshwclmulmovbe;
mod vaesclmulavx2;

#[derive(Clone)]
pub(super) struct Key(DynKey);

impl Key {
    pub(super) fn new(
        key: aes::KeyBytes,
        cpu_features: cpu::Features,
    ) -> Result<Self, error::Unspecified> {
        Ok(Self(DynKey::new(key, cpu_features)?))
    }
}

#[derive(Clone)]
enum DynKey {
    #[cfg(target_arch = "x86_64")]
    VAesClMulAvx2(Combo<aes::hw::Key, gcm::vclmulavx2::Key>),

    #[cfg(target_arch = "x86_64")]
    AesHwClMulAvxMovbe(Combo<aes::hw::Key, gcm::clmulavxmovbe::Key>),

    #[cfg(any(
        all(target_arch = "aarch64", target_endian = "little"),
        target_arch = "x86",
        target_arch = "x86_64"
    ))]
    AesHwClMul(Combo<aes::hw::Key, gcm::clmul::Key>),

    #[cfg(any(
        all(target_arch = "aarch64", target_endian = "little"),
        all(target_arch = "arm", target_endian = "little")
    ))]
    Simd(Combo<aes::vp::Key, gcm::neon::Key>),

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    Simd(Combo<aes::vp::Key, gcm::fallback::Key>),

    Fallback(Combo<aes::fallback::Key, gcm::fallback::Key>),
}

impl DynKey {
    fn new(key: aes::KeyBytes, cpu: cpu::Features) -> Result<Self, error::Unspecified> {
        let cpu = cpu.values();

        #[cfg(target_arch = "x86_64")]
        if let Some((aes, gcm)) = cpu.get_feature() {
            // 14.3.1 Detection of VEX-Encoded AES and VPCLMULQDQ
            let aes_key = aes::hw::Key::new(key, aes, cpu.get_feature())?;
            let gcm_key_value = derive_gcm_key_value(&aes_key);
            let combo = if let Some(cpu) = cpu.get_feature() {
                let gcm_key = gcm::vclmulavx2::Key::new(gcm_key_value, cpu);
                Self::VAesClMulAvx2(Combo { aes_key, gcm_key })
            } else if let Some(cpu) = cpu.get_feature() {
                let gcm_key = gcm::clmulavxmovbe::Key::new(gcm_key_value, cpu);
                Self::AesHwClMulAvxMovbe(Combo { aes_key, gcm_key })
            } else {
                let gcm_key = gcm::clmul::Key::new(gcm_key_value, gcm);
                Self::AesHwClMul(Combo { aes_key, gcm_key })
            };
            return Ok(combo);
        }

        // x86_64 is handled above.
        #[cfg(any(
            all(target_arch = "aarch64", target_endian = "little"),
            target_arch = "x86"
        ))]
        if let (Some(aes), Some(gcm)) = (cpu.get_feature(), cpu.get_feature()) {
            let aes_key = aes::hw::Key::new(key, aes, cpu.get_feature())?;
            let gcm_key_value = derive_gcm_key_value(&aes_key);
            let gcm_key = gcm::clmul::Key::new(gcm_key_value, gcm);
            return Ok(Self::AesHwClMul(Combo { aes_key, gcm_key }));
        }

        #[cfg(any(
            all(target_arch = "aarch64", target_endian = "little"),
            all(target_arch = "arm", target_endian = "little")
        ))]
        if let Some(cpu) = cpu.get_feature() {
            return Self::new_neon(key, cpu);
        }

        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        if let Some(cpu) = cpu.get_feature() {
            return Self::new_ssse3(key, cpu);
        }

        let _ = cpu;
        Self::new_fallback(key)
    }

    #[cfg(any(
        all(target_arch = "aarch64", target_endian = "little"),
        all(target_arch = "arm", target_endian = "little")
    ))]
    #[cfg_attr(target_arch = "aarch64", inline(never))]
    fn new_neon(key: aes::KeyBytes, cpu: cpu::arm::Neon) -> Result<Self, error::Unspecified> {
        let aes_key = aes::vp::Key::new(key, cpu)?;
        let gcm_key_value = derive_gcm_key_value(&aes_key);
        let gcm_key = gcm::neon::Key::new(gcm_key_value, cpu);
        Ok(Self::Simd(Combo { aes_key, gcm_key }))
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[inline(never)]
    fn new_ssse3(
        key: aes::KeyBytes,
        cpu: aes::vp::RequiredCpuFeatures,
    ) -> Result<Self, error::Unspecified> {
        let aes_key = aes::vp::Key::new(key, cpu)?;
        let gcm_key_value = derive_gcm_key_value(&aes_key);
        let gcm_key = gcm::fallback::Key::new(gcm_key_value);
        Ok(Self::Simd(Combo { aes_key, gcm_key }))
    }

    #[cfg_attr(
        any(
            all(target_arch = "aarch64", target_endian = "little"),
            all(target_arch = "arm", target_endian = "little"),
            target_arch = "x86",
            target_arch = "x86_64",
        ),
        inline(never)
    )]
    fn new_fallback(key: aes::KeyBytes) -> Result<Self, error::Unspecified> {
        let aes_key = aes::fallback::Key::new(key)?;
        let gcm_key_value = derive_gcm_key_value(&aes_key);
        let gcm_key = gcm::fallback::Key::new(gcm_key_value);
        Ok(Self::Fallback(Combo { aes_key, gcm_key }))
    }
}

fn derive_gcm_key_value(aes_key: &impl aes::EncryptBlock) -> gcm::KeyValue {
    gcm::KeyValue::new(aes_key.encrypt_block(ZERO_BLOCK))
}

const CHUNK_BLOCKS: usize = 3 * 1024 / 16;

#[inline(never)]
pub(super) fn seal(
    Key(key): &Key,
    nonce: Nonce,
    aad: Aad<&[u8]>,
    in_out: &mut [u8],
) -> Result<Tag, error::Unspecified> {
    let mut ctr = Counter::one(nonce);
    let tag_iv = ctr.increment();

    match key {
        #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
        DynKey::AesHwClMul(c) => {
            seal_whole_partial(c, aad, in_out, ctr, tag_iv, aarch64::seal_whole)
        }

        #[cfg(target_arch = "x86_64")]
        DynKey::VAesClMulAvx2(c) => seal_whole_partial(
            c,
            aad,
            in_out,
            ctr,
            tag_iv,
            vaesclmulavx2::seal_whole_vaes_clmul_avx2,
        ),

        #[cfg(target_arch = "x86_64")]
        DynKey::AesHwClMulAvxMovbe(Combo { aes_key, gcm_key }) => {
            aeshwclmulmovbe::seal(aes_key, gcm_key, ctr, tag_iv, aad, in_out)
        }

        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        DynKey::AesHwClMul(c) => seal_strided(c, aad, in_out, ctr, tag_iv),

        #[cfg(any(
            all(target_arch = "aarch64", target_endian = "little"),
            all(target_arch = "arm", target_endian = "little"),
            target_arch = "x86_64",
            target_arch = "x86"
        ))]
        DynKey::Simd(c) => seal_strided(c, aad, in_out, ctr, tag_iv),

        DynKey::Fallback(c) => seal_strided(c, aad, in_out, ctr, tag_iv),
    }
}

#[cfg(any(
    all(target_arch = "aarch64", target_endian = "little"),
    target_arch = "x86_64"
))]
fn seal_whole_partial<A: aes::EncryptBlock, G: gcm::UpdateBlock>(
    Combo { aes_key, gcm_key }: &Combo<A, G>,
    aad: Aad<&[u8]>,
    in_out: &mut [u8],
    mut ctr: Counter,
    tag_iv: aes::Iv,
    seal_whole: impl FnOnce(&A, &mut gcm::Context<G>, &mut Counter, slice::AsChunksMut<u8, BLOCK_LEN>),
) -> Result<Tag, error::Unspecified> {
    let mut auth = gcm::Context::new(gcm_key, aad, in_out.len())?;
    let (whole, remainder) = slice::as_chunks_mut(in_out);
    seal_whole(aes_key, &mut auth, &mut ctr, whole);
    let remainder = OverlappingPartialBlock::new(remainder.into())
        .unwrap_or_else(|InputTooLongError { .. }| unreachable!());
    seal_finish(aes_key, auth, remainder, ctr, tag_iv)
}

#[cfg_attr(
    any(
        all(target_arch = "aarch64", target_endian = "little"),
        all(target_arch = "arm", target_endian = "little"),
        target_arch = "x86",
        target_arch = "x86_64"
    ),
    inline(never)
)]
#[cfg_attr(
    any(
        all(target_arch = "aarch64", target_endian = "little"),
        target_arch = "x86_64"
    ),
    cold
)]
fn seal_strided<
    A: aes::EncryptBlock + aes::EncryptCtr32,
    G: gcm::UpdateBlock + gcm::UpdateBlocks,
>(
    Combo { aes_key, gcm_key }: &Combo<A, G>,
    aad: Aad<&[u8]>,
    in_out: &mut [u8],
    mut ctr: Counter,
    tag_iv: aes::Iv,
) -> Result<Tag, error::Unspecified> {
    let mut auth = gcm::Context::new(gcm_key, aad, in_out.len())?;

    let (mut whole, remainder) = slice::as_chunks_mut(in_out);

    for mut chunk in whole.chunks_mut::<CHUNK_BLOCKS>() {
        aes_key.ctr32_encrypt_within(chunk.as_flattened_mut().into(), &mut ctr);
        auth.update_blocks(chunk.as_ref());
    }

    let remainder = OverlappingPartialBlock::new(remainder.into())
        .unwrap_or_else(|InputTooLongError { .. }| unreachable!());
    seal_finish(aes_key, auth, remainder, ctr, tag_iv)
}

fn seal_finish<A: aes::EncryptBlock, G: gcm::UpdateBlock>(
    aes_key: &A,
    mut auth: gcm::Context<G>,
    remainder: OverlappingPartialBlock<'_>,
    ctr: Counter,
    tag_iv: aes::Iv,
) -> Result<Tag, error::Unspecified> {
    let remainder_len = remainder.len();
    if remainder_len > 0 {
        let mut input = ZERO_BLOCK;
        overwrite_at_start(&mut input, remainder.input());
        let mut output = aes_key.encrypt_iv_xor_block(ctr.into(), input);
        output[remainder_len..].fill(0);
        auth.update_block(output);
        remainder.overwrite_at_start(output);
    }

    Ok(finish(aes_key, auth, tag_iv))
}

#[inline(never)]
pub(super) fn open(
    Key(key): &Key,
    nonce: Nonce,
    aad: Aad<&[u8]>,
    in_out_slice: &mut [u8],
    src: RangeFrom<usize>,
) -> Result<Tag, error::Unspecified> {
    let mut ctr = Counter::one(nonce);
    let tag_iv = ctr.increment();

    match key {
        #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
        DynKey::AesHwClMul(c) => {
            open_whole_partial(c, aad, in_out_slice, src, ctr, tag_iv, aarch64::open_whole)
        }

        #[cfg(target_arch = "x86_64")]
        DynKey::VAesClMulAvx2(c) => open_whole_partial(
            c,
            aad,
            in_out_slice,
            src,
            ctr,
            tag_iv,
            vaesclmulavx2::open_whole_vaes_clmul_avx2,
        ),

        #[cfg(target_arch = "x86_64")]
        DynKey::AesHwClMulAvxMovbe(Combo { aes_key, gcm_key }) => {
            aeshwclmulmovbe::open(aes_key, gcm_key, ctr, tag_iv, aad, in_out_slice, src)
        }

        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        DynKey::AesHwClMul(c) => open_strided(c, aad, in_out_slice, src, ctr, tag_iv),

        #[cfg(any(
            all(target_arch = "aarch64", target_endian = "little"),
            all(target_arch = "arm", target_endian = "little"),
            target_arch = "x86_64",
            target_arch = "x86"
        ))]
        DynKey::Simd(c) => open_strided(c, aad, in_out_slice, src, ctr, tag_iv),

        DynKey::Fallback(c) => open_strided(c, aad, in_out_slice, src, ctr, tag_iv),
    }
}

#[cfg(any(
    all(target_arch = "aarch64", target_endian = "little"),
    target_arch = "x86_64"
))]
fn open_whole_partial<A: aes::EncryptBlock, G: gcm::UpdateBlock>(
    Combo { aes_key, gcm_key }: &Combo<A, G>,
    aad: Aad<&[u8]>,
    in_out_slice: &mut [u8],
    src: RangeFrom<usize>,
    mut ctr: Counter,
    tag_iv: aes::Iv,
    open_whole: impl FnOnce(&A, &mut gcm::Context<G>, Overlapping, &mut Counter),
) -> Result<Tag, error::Unspecified> {
    let in_out = Overlapping::new(in_out_slice, src.clone()).map_err(error::erase::<IndexError>)?;
    let mut auth = gcm::Context::new(gcm_key, aad, in_out.len())?;

    let remainder_len = in_out.len() % BLOCK_LEN;

    let in_out_slice_len = in_out_slice.len();
    let whole_in_out_slice = &mut in_out_slice[..(in_out_slice_len - remainder_len)];
    let whole = Overlapping::new(whole_in_out_slice, src.clone())
        .unwrap_or_else(|IndexError { .. }| unreachable!());
    let whole_len = whole.len();
    open_whole(aes_key, &mut auth, whole, &mut ctr);

    let remainder = &mut in_out_slice[whole_len..];
    let remainder =
        Overlapping::new(remainder, src).unwrap_or_else(|IndexError { .. }| unreachable!());
    let remainder = OverlappingPartialBlock::new(remainder)
        .unwrap_or_else(|InputTooLongError { .. }| unreachable!());
    open_finish(aes_key, auth, remainder, ctr, tag_iv)
}

#[cfg_attr(
    any(
        all(
            any(
                all(target_arch = "aarch64", target_endian = "little"),
                all(target_arch = "arm", target_endian = "little")
            ),
            target_feature = "neon"
        ),
        all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse"
        )
    ),
    inline(never)
)]
#[cfg_attr(
    any(
        all(target_arch = "aarch64", target_endian = "little"),
        target_arch = "x86_64"
    ),
    cold
)]
fn open_strided<
    A: aes::EncryptBlock + aes::EncryptCtr32,
    G: gcm::UpdateBlock + gcm::UpdateBlocks,
>(
    Combo { aes_key, gcm_key }: &Combo<A, G>,
    aad: Aad<&[u8]>,
    in_out_slice: &mut [u8],
    src: RangeFrom<usize>,
    mut ctr: Counter,
    tag_iv: aes::Iv,
) -> Result<Tag, error::Unspecified> {
    let in_out = Overlapping::new(in_out_slice, src.clone()).map_err(error::erase::<IndexError>)?;
    let input = in_out.input();
    let input_len = input.len();

    let mut auth = gcm::Context::new(gcm_key, aad, input_len)?;

    let remainder_len = input_len % BLOCK_LEN;
    let whole_len = input_len - remainder_len;
    let in_prefix_len = src.start;

    {
        let mut chunk_len = CHUNK_BLOCKS * BLOCK_LEN;
        let mut output = 0;
        let mut input = in_prefix_len;
        loop {
            if whole_len - output < chunk_len {
                chunk_len = whole_len - output;
            }

            let ciphertext = &in_out_slice[input..][..chunk_len];
            let (ciphertext, leftover) = slice::as_chunks(ciphertext);
            debug_assert_eq!(leftover.len(), 0);
            if ciphertext.is_empty() {
                break;
            }
            auth.update_blocks(ciphertext);

            let chunk = Overlapping::new(
                &mut in_out_slice[output..][..(chunk_len + in_prefix_len)],
                in_prefix_len..,
            )
            .map_err(error::erase::<IndexError>)?;
            aes_key.ctr32_encrypt_within(chunk, &mut ctr);
            output += chunk_len;
            input += chunk_len;
        }
    }

    let in_out = Overlapping::new(&mut in_out_slice[whole_len..], src)
        .unwrap_or_else(|IndexError { .. }| unreachable!());
    let in_out = OverlappingPartialBlock::new(in_out)
        .unwrap_or_else(|InputTooLongError { .. }| unreachable!());

    open_finish(aes_key, auth, in_out, ctr, tag_iv)
}

fn open_finish<A: aes::EncryptBlock, G: gcm::UpdateBlock>(
    aes_key: &A,
    mut auth: gcm::Context<G>,
    remainder: OverlappingPartialBlock<'_>,
    ctr: Counter,
    tag_iv: aes::Iv,
) -> Result<Tag, error::Unspecified> {
    if remainder.len() > 0 {
        let mut input = ZERO_BLOCK;
        overwrite_at_start(&mut input, remainder.input());
        auth.update_block(input);
        remainder.overwrite_at_start(aes_key.encrypt_iv_xor_block(ctr.into(), input));
    }
    Ok(finish(aes_key, auth, tag_iv))
}

fn finish<A: aes::EncryptBlock, G: gcm::UpdateBlock>(
    aes_key: &A,
    gcm_ctx: gcm::Context<G>,
    tag_iv: aes::Iv,
) -> Tag {
    // Finalize the tag and return it.
    gcm_ctx.pre_finish(|pre_tag| Tag(aes_key.encrypt_iv_xor_block(tag_iv, pre_tag)))
}

pub(super) const MAX_IN_OUT_LEN: usize = super::max_input_len(BLOCK_LEN, 2);

// [NIST SP800-38D] Section 5.2.1.1. Note that [RFC 5116 Section 5.1] and
// [RFC 5116 Section 5.2] have an off-by-one error in `P_MAX`.
//
// [NIST SP800-38D]:
//    http://nvlpubs.nist.gov/nistpubs/Legacy/SP/nistspecialpublication800-38d.pdf
// [RFC 5116 Section 5.1]: https://tools.ietf.org/html/rfc5116#section-5.1
// [RFC 5116 Section 5.2]: https://tools.ietf.org/html/rfc5116#section-5.2
const _MAX_INPUT_LEN_BOUNDED_BY_NIST: () =
    assert!(MAX_IN_OUT_LEN == usize_from_u64_saturated(((1u64 << 39) - 256) / 8));

#[derive(Copy, Clone)]
pub(super) struct Combo<Aes, Gcm> {
    pub(super) aes_key: Aes,
    pub(super) gcm_key: Gcm,
}
