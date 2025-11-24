// Copyright 2025 Brian Smith.
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

use crate::{
    error::LenMismatchError,
    limb::{Limb, LIMB_BITS},
    polyfill::slice::{self, AsChunksMut},
};
use core::mem::{align_of, size_of};

// Some x86_64 assembly is written under the assumption that some of its
// input data and/or temporary storage is aligned to `MOD_EXP_CTIME_ALIGN`
// bytes, which was/is 64 in OpenSSL.
//
// We use this in the non-X86-64 implementation of exponentiation as well,
// with the hope of converging th two implementations into one.

#[repr(C, align(64))]
pub struct AlignedStorage<const N: usize>([Limb; N]);

const _LIMB_SIZE_DIVIDES_ALIGNMENT: () =
    assert!(align_of::<AlignedStorage<1>>() % size_of::<Limb>() == 0);

pub const LIMBS_PER_CHUNK: usize = 512 / LIMB_BITS;

impl<const N: usize> AlignedStorage<N> {
    pub fn zeroed() -> Self {
        assert_eq!(N % LIMBS_PER_CHUNK, 0); // TODO: const.
        Self([0; N])
    }

    // The result will have every chunk aligned on a 64 byte boundary.
    pub fn aligned_chunks_mut(
        &mut self,
        num_entries: usize,
        chunks_per_entry: usize,
    ) -> Result<AsChunksMut<Limb, LIMBS_PER_CHUNK>, LenMismatchError> {
        let total_limbs = num_entries * chunks_per_entry * LIMBS_PER_CHUNK;
        let len = self.0.len();
        let flattened = self
            .0
            .get_mut(..total_limbs)
            .ok_or_else(|| LenMismatchError::new(len))?;
        match slice::as_chunks_mut(flattened) {
            (chunks, []) => Ok(chunks),
            (_, r) => Err(LenMismatchError::new(r.len())),
        }
    }
}
