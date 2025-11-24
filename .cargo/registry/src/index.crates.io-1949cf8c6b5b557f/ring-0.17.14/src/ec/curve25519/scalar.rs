// Copyright 2015-2019 Brian Smith.
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
    arithmetic::limbs_from_hex,
    digest, error, limb,
    polyfill::slice::{self, AsChunks},
};
use core::array;

#[repr(transparent)]
pub struct Scalar([u8; SCALAR_LEN]);

pub const SCALAR_LEN: usize = 32;

impl Scalar {
    // Constructs a `Scalar` from `bytes`, failing if `bytes` encodes a scalar
    // that is not in the range [0, n).
    pub fn from_bytes_checked(bytes: [u8; SCALAR_LEN]) -> Result<Self, error::Unspecified> {
        const ORDER: [limb::Limb; SCALAR_LEN / limb::LIMB_BYTES] =
            limbs_from_hex("1000000000000000000000000000000014def9dea2f79cd65812631a5cf5d3ed");
        let order = ORDER.map(limb::Limb::from);

        let (limbs_as_bytes, _empty): (AsChunks<u8, { limb::LIMB_BYTES }>, _) =
            slice::as_chunks(&bytes);
        debug_assert!(_empty.is_empty());
        let limbs: [limb::Limb; SCALAR_LEN / limb::LIMB_BYTES] =
            array::from_fn(|i| limb::Limb::from_le_bytes(limbs_as_bytes[i]));
        limb::verify_limbs_less_than_limbs_leak_bit(&limbs, &order)?;

        Ok(Self(bytes))
    }

    // Constructs a `Scalar` from `digest` reduced modulo n.
    pub fn from_sha512_digest_reduced(digest: digest::Digest) -> Self {
        prefixed_extern! {
            fn x25519_sc_reduce(s: &mut UnreducedScalar);
        }
        let mut unreduced = [0u8; digest::SHA512_OUTPUT_LEN];
        unreduced.copy_from_slice(digest.as_ref());
        unsafe { x25519_sc_reduce(&mut unreduced) };
        Self((&unreduced[..SCALAR_LEN]).try_into().unwrap())
    }
}

#[repr(transparent)]
pub struct MaskedScalar([u8; SCALAR_LEN]);

impl MaskedScalar {
    pub fn from_bytes_masked(bytes: [u8; SCALAR_LEN]) -> Self {
        prefixed_extern! {
            fn x25519_sc_mask(a: &mut [u8; SCALAR_LEN]);
        }
        let mut r = Self(bytes);
        unsafe { x25519_sc_mask(&mut r.0) };
        r
    }
}

impl From<MaskedScalar> for Scalar {
    fn from(MaskedScalar(scalar): MaskedScalar) -> Self {
        Self(scalar)
    }
}

type UnreducedScalar = [u8; UNREDUCED_SCALAR_LEN];
const UNREDUCED_SCALAR_LEN: usize = SCALAR_LEN * 2;
