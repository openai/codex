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

#![cfg(all(target_arch = "aarch64", target_endian = "little"))]

use super::super::super::{inout::AliasingSlices3 as _, n0::N0, LimbSliceError, MAX_LIMBS};
use crate::{
    c,
    limb::Limb,
    polyfill::slice::{AsChunks, AsChunksMut},
};
use core::num::NonZeroUsize;

#[inline]
pub(in super::super::super) fn sqr_mont5(
    mut in_out: AsChunksMut<Limb, 8>,
    n: AsChunks<Limb, 8>,
    n0: &N0,
) -> Result<(), LimbSliceError> {
    prefixed_extern! {
        // `r` and/or 'a' may alias.
        // XXX: BoringSSL (kinda, implicitly) declares this to return `int`.
        // `num` must be a non-zero multiple of 8.
        fn bn_sqr8x_mont(
            rp: *mut Limb,
            ap: *const Limb,
            ap_again: *const Limb,
            np: *const Limb,
            n0: &N0,
            num: c::NonZero_size_t);
    }

    let in_out = in_out.as_flattened_mut();
    let n = n.as_flattened();
    let num_limbs = NonZeroUsize::new(n.len()).ok_or_else(|| LimbSliceError::too_short(n.len()))?;

    // Avoid stack overflow from the alloca inside.
    if num_limbs.get() > MAX_LIMBS {
        return Err(LimbSliceError::too_long(num_limbs.get()));
    }

    in_out
        .with_non_dangling_non_null_pointers_rab(num_limbs, |r, a, a_again| {
            let n = n.as_ptr(); // Non-dangling because num_limbs > 0.
            unsafe { bn_sqr8x_mont(r, a, a_again, n, n0, num_limbs) };
        })
        .map_err(LimbSliceError::len_mismatch)
}
