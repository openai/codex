// Copyright 2018-2024 Brian Smith.
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

use self::ffi::{Block, BLOCK_LEN, ZERO_BLOCK};
use super::{aes_gcm, Aad};
use crate::{
    bits::{BitLength, FromByteLen as _},
    error::{self, InputTooLongError},
    polyfill::{slice::AsChunks, sliceutil::overwrite_at_start, NotSend},
};
use cfg_if::cfg_if;

pub(super) use ffi::KeyValue;

cfg_if! {
    if #[cfg(any(all(target_arch = "aarch64", target_endian = "little"), target_arch = "x86_64"))] {
        pub(super) use self::ffi::{HTable, Xi};
    } else {
        use self::ffi::{HTable, Xi};
    }
}

#[macro_use]
mod ffi;

pub(super) mod clmul;
pub(super) mod clmulavxmovbe;
pub(super) mod fallback;
pub(super) mod neon;
pub(super) mod vclmulavx2;

pub(super) struct Context<'key, K> {
    Xi: Xi,
    key: &'key K,
    aad_len: BitLength<u64>,
    in_out_len: BitLength<u64>,
    _not_send: NotSend,
}

impl<'key, K: UpdateBlock> Context<'key, K> {
    #[inline(always)]
    pub(crate) fn new(
        key: &'key K,
        aad: Aad<&[u8]>,
        in_out_len: usize,
    ) -> Result<Self, error::Unspecified> {
        if in_out_len > aes_gcm::MAX_IN_OUT_LEN {
            return Err(error::Unspecified);
        }
        let in_out_len =
            BitLength::from_byte_len(in_out_len).map_err(error::erase::<InputTooLongError>)?;
        let aad_len = BitLength::from_byte_len(aad.as_ref().len())
            .map_err(error::erase::<InputTooLongError>)?;

        // NIST SP800-38D Section 5.2.1.1 says that the maximum AAD length is
        // 2**64 - 1 bits, i.e. BitLength<u64>::MAX, so we don't need to do an
        // explicit check here.

        let mut ctx = Self {
            Xi: Xi(ZERO_BLOCK),
            key,
            aad_len,
            in_out_len,
            _not_send: NotSend::VALUE,
        };

        for ad in aad.0.chunks(BLOCK_LEN) {
            let mut block = ZERO_BLOCK;
            overwrite_at_start(&mut block, ad);
            ctx.update_block(block);
        }

        Ok(ctx)
    }
}

#[cfg(all(
    target_arch = "aarch64",
    target_endian = "little",
    target_pointer_width = "64"
))]
impl<K> Context<'_, K> {
    pub(super) fn in_out_whole_block_bits(&self) -> BitLength<usize> {
        use crate::polyfill::usize_from_u64;
        const WHOLE_BLOCK_BITS_MASK: usize = !0b111_1111;
        #[allow(clippy::assertions_on_constants)]
        const _WHOLE_BLOCK_BITS_MASK_CORRECT: () =
            assert!(WHOLE_BLOCK_BITS_MASK == !((BLOCK_LEN * 8) - 1));
        BitLength::from_bits(usize_from_u64(self.in_out_len.as_bits()) & WHOLE_BLOCK_BITS_MASK)
    }
}

#[cfg(all(target_arch = "aarch64", target_endian = "little"))]
/// Access to `inner` for the integrated AES-GCM implementations only.
impl Context<'_, clmul::Key> {
    #[inline]
    pub(super) fn inner(&mut self) -> (&HTable, &mut Xi) {
        (&self.key.inner(), &mut self.Xi)
    }
}

#[cfg(target_arch = "x86_64")]
impl Context<'_, clmulavxmovbe::Key> {
    /// Access to `inner` for the integrated AES-GCM implementations only.
    #[inline]
    pub(super) fn inner(&mut self) -> (&HTable, &mut Xi) {
        (self.key.inner(), &mut self.Xi)
    }
}

#[cfg(target_arch = "x86_64")]
impl Context<'_, vclmulavx2::Key> {
    /// Access to `inner` for the integrated AES-GCM implementations only.
    #[inline]
    pub(super) fn inner(&mut self) -> (&HTable, &mut Xi) {
        (self.key.inner(), &mut self.Xi)
    }
}

impl<K: UpdateBlocks> Context<'_, K> {
    #[inline(always)]
    pub fn update_blocks(&mut self, input: AsChunks<u8, BLOCK_LEN>) {
        self.key.update_blocks(&mut self.Xi, input);
    }
}

impl<K: UpdateBlock> Context<'_, K> {
    pub fn update_block(&mut self, a: Block) {
        self.key.update_block(&mut self.Xi, a);
    }

    #[inline(always)]
    pub(super) fn pre_finish<F>(mut self, f: F) -> super::Tag
    where
        F: FnOnce(Block) -> super::Tag,
    {
        let mut block = [0u8; BLOCK_LEN];
        let (alen, clen) = block.split_at_mut(BLOCK_LEN / 2);
        alen.copy_from_slice(&BitLength::<u64>::to_be_bytes(self.aad_len));
        clen.copy_from_slice(&BitLength::<u64>::to_be_bytes(self.in_out_len));
        self.update_block(block);
        f(self.Xi.0)
    }
}

pub(super) trait UpdateBlock {
    fn update_block(&self, xi: &mut Xi, a: Block);
}

pub(super) trait UpdateBlocks {
    fn update_blocks(&self, xi: &mut Xi, input: AsChunks<u8, BLOCK_LEN>);
}
