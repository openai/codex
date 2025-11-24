// Copyright 2015-2021 Brian Smith.
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

use super::{PublicExponent, PublicModulus, N, PUBLIC_KEY_PUBLIC_MODULUS_MAX_LEN};
use crate::{
    arithmetic::bigint,
    bits, cpu, error,
    io::{self, der, der_writer},
    limb::LIMB_BYTES,
};
use alloc::boxed::Box;
use core::num::NonZeroU64;

/// An RSA Public Key.
#[derive(Clone)]
pub struct PublicKey {
    inner: Inner,
    serialized: Box<[u8]>,
}

derive_debug_self_as_ref_hex_bytes!(PublicKey);

impl PublicKey {
    pub(super) fn from_modulus_and_exponent(
        n: untrusted::Input,
        e: untrusted::Input,
        n_min_bits: bits::BitLength,
        n_max_bits: bits::BitLength,
        e_min_value: PublicExponent,
        cpu_features: cpu::Features,
    ) -> Result<Self, error::KeyRejected> {
        let inner = Inner::from_modulus_and_exponent(
            n,
            e,
            n_min_bits,
            n_max_bits,
            e_min_value,
            cpu_features,
        )?;

        let n_bytes = n;
        let e_bytes = e;

        // TODO: Remove this re-parsing, and stop allocating this here.
        // Instead we should serialize on demand without allocation, from
        // `Modulus::be_bytes()` and `Exponent::be_bytes()`. Once this is
        // fixed, merge `Inner` back into `PublicKey`.
        let n_bytes = io::Positive::from_be_bytes(n_bytes)
            .map_err(|_: error::Unspecified| error::KeyRejected::unexpected_error())?;
        let e_bytes = io::Positive::from_be_bytes(e_bytes)
            .map_err(|_: error::Unspecified| error::KeyRejected::unexpected_error())?;
        let serialized = der_writer::write_all(der::Tag::Sequence, &|output| {
            der_writer::write_positive_integer(output, &n_bytes)?;
            der_writer::write_positive_integer(output, &e_bytes)
        })
        .map_err(|_: io::TooLongError| error::KeyRejected::unexpected_error())?;

        Ok(Self { inner, serialized })
    }

    /// The length, in bytes, of the public modulus.
    ///
    /// The modulus length is rounded up to a whole number of bytes if its
    /// bit length isn't a multiple of 8.
    pub fn modulus_len(&self) -> usize {
        self.inner.n().len_bits().as_usize_bytes_rounded_up()
    }

    pub(super) fn inner(&self) -> &Inner {
        &self.inner
    }
}

/// `PublicKey` but without any superfluous allocations, optimized for one-shot
/// RSA signature verification.
#[derive(Clone)]
pub(crate) struct Inner {
    n: PublicModulus,
    e: PublicExponent,
}

impl Inner {
    pub(super) fn from_modulus_and_exponent(
        n: untrusted::Input,
        e: untrusted::Input,
        n_min_bits: bits::BitLength,
        n_max_bits: bits::BitLength,
        e_min_value: PublicExponent,
        cpu_features: cpu::Features,
    ) -> Result<Self, error::KeyRejected> {
        // This is an incomplete implementation of NIST SP800-56Br1 Section
        // 6.4.2.2, "Partial Public-Key Validation for RSA." That spec defers
        // to NIST SP800-89 Section 5.3.3, "(Explicit) Partial Public Key
        // Validation for RSA," "with the caveat that the length of the modulus
        // shall be a length that is specified in this Recommendation." In
        // SP800-89, two different sets of steps are given, one set numbered,
        // and one set lettered. TODO: Document this in the end-user
        // documentation for RSA keys.

        let n = PublicModulus::from_be_bytes(n, n_min_bits..=n_max_bits, cpu_features)?;

        let e = PublicExponent::from_be_bytes(e, e_min_value)?;

        // If `n` is less than `e` then somebody has probably accidentally swapped
        // them. The largest acceptable `e` is smaller than the smallest acceptable
        // `n`, so no additional checks need to be done.

        // XXX: Steps 4 & 5 / Steps d, e, & f are not implemented. This is also the
        // case in most other commonly-used crypto libraries.

        Ok(Self { n, e })
    }

    /// The public modulus.
    #[inline]
    pub(super) fn n(&self) -> &PublicModulus {
        &self.n
    }

    /// The public exponent.
    #[inline]
    pub(super) fn e(&self) -> PublicExponent {
        self.e
    }

    /// Calculates base**e (mod n), filling the first part of `out_buffer` with
    /// the result.
    ///
    /// This is constant-time with respect to the value in `base` (only).
    ///
    /// The result will be a slice of the encoded bytes of the result within
    /// `out_buffer`, if successful.
    pub(super) fn exponentiate<'out>(
        &self,
        base: untrusted::Input,
        out_buffer: &'out mut [u8; PUBLIC_KEY_PUBLIC_MODULUS_MAX_LEN],
        cpu_features: cpu::Features,
    ) -> Result<&'out [u8], error::Unspecified> {
        let n = &self.n.value(cpu_features);

        // The encoded value of the base must be the same length as the modulus,
        // in bytes.
        if base.len() != self.n.len_bits().as_usize_bytes_rounded_up() {
            return Err(error::Unspecified);
        }

        // RFC 8017 Section 5.2.2: RSAVP1.

        // Step 1.
        let s = bigint::Elem::from_be_bytes_padded(base, n)?;
        if s.is_zero() {
            return Err(error::Unspecified);
        }

        // Step 2.
        let m = n.alloc_zero();
        let m = self.exponentiate_elem(m, &s, cpu_features);

        // Step 3.
        Ok(fill_be_bytes_n(m, self.n.len_bits(), out_buffer))
    }

    /// Calculates base**e (mod n).
    ///
    /// This is constant-time with respect to `base` only.
    pub(super) fn exponentiate_elem(
        &self,
        out: bigint::Storage<N>,
        base: &bigint::Elem<N>,
        cpu_features: cpu::Features,
    ) -> bigint::Elem<N> {
        // The exponent was already checked to be at least 3.
        let exponent_without_low_bit = NonZeroU64::try_from(self.e.value().get() & !1).unwrap();
        // The exponent was already checked to be odd.
        debug_assert_ne!(exponent_without_low_bit, self.e.value());

        let n = &self.n.value(cpu_features);

        let tmp = n.alloc_zero();
        let base_r = bigint::elem_mul_into(tmp, self.n.oneRR(), base, n);

        // During RSA public key operations the exponent is almost always either
        // 65537 (0b10000000000000001) or 3 (0b11), both of which have a Hamming
        // weight of 2. The maximum bit length and maximum Hamming weight of the
        // exponent is bounded by the value of `PublicExponent::MAX`.
        let acc = bigint::elem_exp_vartime(out, base_r, exponent_without_low_bit, n);

        // Now do the multiplication for the low bit and convert out of the Montgomery domain.
        bigint::elem_mul(base, acc, n)
    }
}

// XXX: Refactor `signature::KeyPair` to get rid of this.
impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        &self.serialized
    }
}

/// Returns the big-endian representation of `elem` that is
/// the same length as the minimal-length big-endian representation of
/// the modulus `n`.
///
/// `n_bits` must be the bit length of the public modulus `n`.
fn fill_be_bytes_n(
    elem: bigint::Elem<N>,
    n_bits: bits::BitLength,
    out: &mut [u8; PUBLIC_KEY_PUBLIC_MODULUS_MAX_LEN],
) -> &[u8] {
    let n_bytes = n_bits.as_usize_bytes_rounded_up();
    let n_bytes_padded = ((n_bytes + (LIMB_BYTES - 1)) / LIMB_BYTES) * LIMB_BYTES;
    let out = &mut out[..n_bytes_padded];
    elem.fill_be_bytes(out);
    let (padding, out) = out.split_at(n_bytes_padded - n_bytes);
    assert!(padding.iter().all(|&b| b == 0));
    out
}
