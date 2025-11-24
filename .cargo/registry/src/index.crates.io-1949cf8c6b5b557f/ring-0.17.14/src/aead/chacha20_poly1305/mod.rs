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
    chacha::{self, Counter, Overlapping},
    poly1305, Aad, Nonce, Tag,
};
use crate::{
    cpu,
    error::InputTooLongError,
    polyfill::{slice, sliceutil, u64_from_usize, usize_from_u64_saturated},
};
use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(any(
            all(target_arch = "aarch64", target_endian = "little"),
            target_arch = "x86_64"))] {
        use cpu::GetFeature as _;
        mod integrated;
    }
}

pub(super) const KEY_LEN: usize = chacha::KEY_LEN;

const MAX_IN_OUT_LEN: usize = super::max_input_len(64, 1);
// https://tools.ietf.org/html/rfc8439#section-2.8
const _MAX_IN_OUT_LEN_BOUNDED_BY_RFC: () =
    assert!(MAX_IN_OUT_LEN == usize_from_u64_saturated(274_877_906_880u64));

#[derive(Clone)]
pub(super) struct Key(chacha::Key);

impl Key {
    pub(super) fn new(value: [u8; KEY_LEN]) -> Self {
        Self(chacha::Key::new(value))
    }
}

pub(super) fn seal(
    key: &Key,
    nonce: Nonce,
    aad: Aad<&[u8]>,
    in_out: &mut [u8],
    cpu: cpu::Features,
) -> Result<Tag, InputTooLongError> {
    #[cfg(any(
        all(target_arch = "aarch64", target_endian = "little"),
        target_arch = "x86_64"
    ))]
    if let Some(required) = cpu.get_feature() {
        return integrated::seal(key, nonce, aad, in_out, required, cpu.get_feature());
    }

    seal_fallback(key, nonce, aad, in_out, cpu)
}

pub(super) fn seal_fallback(
    Key(chacha20_key): &Key,
    nonce: Nonce,
    aad: Aad<&[u8]>,
    in_out: &mut [u8],
    cpu: cpu::Features,
) -> Result<Tag, InputTooLongError> {
    let (counter, poly1305_key) = begin(chacha20_key, nonce, aad, in_out, cpu)?;
    let mut auth = poly1305::Context::from_key(poly1305_key, cpu);

    poly1305_update_padded_16(&mut auth, aad.as_ref());
    chacha20_key.encrypt(counter, in_out.into(), cpu);
    poly1305_update_padded_16(&mut auth, in_out);
    Ok(finish(auth, aad.as_ref().len(), in_out.len()))
}

pub(super) fn open(
    key: &Key,
    nonce: Nonce,
    aad: Aad<&[u8]>,
    in_out: Overlapping<'_>,
    cpu: cpu::Features,
) -> Result<Tag, InputTooLongError> {
    #[cfg(any(
        all(target_arch = "aarch64", target_endian = "little"),
        target_arch = "x86_64"
    ))]
    if let Some(required) = cpu.get_feature() {
        return integrated::open(key, nonce, aad, in_out, required, cpu.get_feature());
    }

    open_fallback(key, nonce, aad, in_out, cpu)
}

pub(super) fn open_fallback(
    Key(chacha20_key): &Key,
    nonce: Nonce,
    aad: Aad<&[u8]>,
    in_out: Overlapping<'_>,
    cpu: cpu::Features,
) -> Result<Tag, InputTooLongError> {
    let (counter, poly1305_key) = begin(chacha20_key, nonce, aad, in_out.input(), cpu)?;
    let mut auth = poly1305::Context::from_key(poly1305_key, cpu);

    poly1305_update_padded_16(&mut auth, aad.as_ref());
    poly1305_update_padded_16(&mut auth, in_out.input());
    let in_out_len = in_out.len();
    chacha20_key.encrypt(counter, in_out, cpu);
    Ok(finish(auth, aad.as_ref().len(), in_out_len))
}

fn check_input_lengths(aad: Aad<&[u8]>, input: &[u8]) -> Result<(), InputTooLongError> {
    if input.len() > MAX_IN_OUT_LEN {
        return Err(InputTooLongError::new(input.len()));
    }

    // RFC 8439 Section 2.8 says the maximum AAD length is 2**64 - 1, which is
    // never larger than usize::MAX, so we don't need an explicit length
    // check.
    const _USIZE_BOUNDED_BY_U64: u64 = u64_from_usize(usize::MAX);
    let _ = aad;

    Ok(())
}

// Also used by chacha20_poly1305_openssh.
pub(super) fn begin(
    key: &chacha::Key,
    nonce: Nonce,
    aad: Aad<&[u8]>,
    input: &[u8],
    cpu: cpu::Features,
) -> Result<(Counter, poly1305::Key), InputTooLongError> {
    check_input_lengths(aad, input)?;

    let mut key_bytes = [0u8; poly1305::KEY_LEN];
    let counter = key.encrypt_single_block_with_ctr_0(nonce, &mut key_bytes, cpu);
    let poly1305_key = poly1305::Key::new(key_bytes);
    Ok((counter, poly1305_key))
}

fn finish(auth: poly1305::Context, aad_len: usize, in_out_len: usize) -> Tag {
    let mut block = [0u8; poly1305::BLOCK_LEN];
    let (alen, clen) = block.split_at_mut(poly1305::BLOCK_LEN / 2);
    alen.copy_from_slice(&u64::to_le_bytes(u64_from_usize(aad_len)));
    clen.copy_from_slice(&u64::to_le_bytes(u64_from_usize(in_out_len)));
    auth.finish(&block)
}

#[inline]
fn poly1305_update_padded_16(ctx: &mut poly1305::Context, input: &[u8]) {
    let (whole, remainder) = slice::as_chunks(input);
    ctx.update(whole);
    if !remainder.is_empty() {
        let mut block = [0u8; poly1305::BLOCK_LEN];
        sliceutil::overwrite_at_start(&mut block, remainder);
        ctx.update_block(block);
    }
}
