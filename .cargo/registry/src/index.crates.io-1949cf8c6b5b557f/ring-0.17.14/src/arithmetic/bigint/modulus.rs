// Copyright 2015-2024 Brian Smith.
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
    super::montgomery::Unencoded, unwrap_impossible_len_mismatch_error, BoxedLimbs, Elem,
    OwnedModulusValue, PublicModulus, Storage, N0,
};
use crate::{
    bits::BitLength,
    cpu, error,
    limb::{self, Limb, LIMB_BITS},
    polyfill::LeadingZerosStripped,
};
use core::marker::PhantomData;

/// The modulus *m* for a ring ℤ/mℤ, along with the precomputed values needed
/// for efficient Montgomery multiplication modulo *m*. The value must be odd
/// and larger than 2. The larger-than-1 requirement is imposed, at least, by
/// the modular inversion code.
pub struct OwnedModulus<M> {
    inner: OwnedModulusValue<M>,

    // n0 * N == -1 (mod r).
    //
    // r == 2**(N0::LIMBS_USED * LIMB_BITS) and LG_LITTLE_R == lg(r). This
    // ensures that we can do integer division by |r| by simply ignoring
    // `N0::LIMBS_USED` limbs. Similarly, we can calculate values modulo `r` by
    // just looking at the lowest `N0::LIMBS_USED` limbs. This is what makes
    // Montgomery multiplication efficient.
    //
    // As shown in Algorithm 1 of "Fast Prime Field Elliptic Curve Cryptography
    // with 256 Bit Primes" by Shay Gueron and Vlad Krasnov, in the loop of a
    // multi-limb Montgomery multiplication of a * b (mod n), given the
    // unreduced product t == a * b, we repeatedly calculate:
    //
    //    t1 := t % r         |t1| is |t|'s lowest limb (see previous paragraph).
    //    t2 := t1*n0*n
    //    t3 := t + t2
    //    t := t3 / r         copy all limbs of |t3| except the lowest to |t|.
    //
    // In the last step, it would only make sense to ignore the lowest limb of
    // |t3| if it were zero. The middle steps ensure that this is the case:
    //
    //                            t3 ==  0 (mod r)
    //                        t + t2 ==  0 (mod r)
    //                   t + t1*n0*n ==  0 (mod r)
    //                       t1*n0*n == -t (mod r)
    //                        t*n0*n == -t (mod r)
    //                          n0*n == -1 (mod r)
    //                            n0 == -1/n (mod r)
    //
    // Thus, in each iteration of the loop, we multiply by the constant factor
    // n0, the negative inverse of n (mod r).
    //
    // TODO(perf): Not all 32-bit platforms actually make use of n0[1]. For the
    // ones that don't, we could use a shorter `R` value and use faster `Limb`
    // calculations instead of double-precision `u64` calculations.
    n0: N0,
}

impl<M: PublicModulus> Clone for OwnedModulus<M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            n0: self.n0,
        }
    }
}

impl<M> OwnedModulus<M> {
    pub(crate) fn from(n: OwnedModulusValue<M>) -> Self {
        // n_mod_r = n % r. As explained in the documentation for `n0`, this is
        // done by taking the lowest `N0::LIMBS_USED` limbs of `n`.
        #[allow(clippy::useless_conversion)]
        let n0 = {
            prefixed_extern! {
                fn bn_neg_inv_mod_r_u64(n: u64) -> u64;
            }

            // XXX: u64::from isn't guaranteed to be constant time.
            let mut n_mod_r: u64 = u64::from(n.limbs()[0]);

            if N0::LIMBS_USED == 2 {
                // XXX: If we use `<< LIMB_BITS` here then 64-bit builds
                // fail to compile because of `deny(exceeding_bitshifts)`.
                debug_assert_eq!(LIMB_BITS, 32);
                n_mod_r |= u64::from(n.limbs()[1]) << 32;
            }
            N0::precalculated(unsafe { bn_neg_inv_mod_r_u64(n_mod_r) })
        };

        Self { inner: n, n0 }
    }

    pub fn to_elem<L>(&self, l: &Modulus<L>) -> Result<Elem<L, Unencoded>, error::Unspecified> {
        self.inner.verify_less_than(l)?;
        let mut limbs = BoxedLimbs::zero(l.limbs().len());
        limbs[..self.inner.limbs().len()].copy_from_slice(self.inner.limbs());
        Ok(Elem {
            limbs,
            encoding: PhantomData,
        })
    }

    pub(crate) fn modulus(&self, cpu_features: cpu::Features) -> Modulus<M> {
        Modulus {
            limbs: self.inner.limbs(),
            n0: self.n0,
            len_bits: self.len_bits(),
            m: PhantomData,
            cpu_features,
        }
    }

    pub fn len_bits(&self) -> BitLength {
        self.inner.len_bits()
    }
}

impl<M: PublicModulus> OwnedModulus<M> {
    pub fn be_bytes(&self) -> LeadingZerosStripped<impl ExactSizeIterator<Item = u8> + Clone + '_> {
        LeadingZerosStripped::new(limb::unstripped_be_bytes(self.inner.limbs()))
    }
}

pub struct Modulus<'a, M> {
    limbs: &'a [Limb],
    n0: N0,
    len_bits: BitLength,
    m: PhantomData<M>,
    cpu_features: cpu::Features,
}

impl<M> Modulus<'_, M> {
    pub(super) fn oneR(&self, out: &mut [Limb]) {
        assert_eq!(self.limbs.len(), out.len());

        let r = self.limbs.len() * LIMB_BITS;

        // out = 2**r - m where m = self.
        limb::limbs_negative_odd(out, self.limbs);

        let lg_m = self.len_bits().as_bits();
        let leading_zero_bits_in_m = r - lg_m;

        // When m's length is a multiple of LIMB_BITS, which is the case we
        // most want to optimize for, then we already have
        // out == 2**r - m == 2**r (mod m).
        if leading_zero_bits_in_m != 0 {
            debug_assert!(leading_zero_bits_in_m < LIMB_BITS);
            // Correct out to 2**(lg m) (mod m). `limbs_negative_odd` flipped
            // all the leading zero bits to ones. Flip them back.
            *out.last_mut().unwrap() &= (!0) >> leading_zero_bits_in_m;

            // Now we have out == 2**(lg m) (mod m). Keep doubling until we get
            // to 2**r (mod m).
            for _ in 0..leading_zero_bits_in_m {
                limb::limbs_double_mod(out, self.limbs)
                    .unwrap_or_else(unwrap_impossible_len_mismatch_error);
            }
        }

        // Now out == 2**r (mod m) == 1*R.
    }

    // TODO: XXX Avoid duplication with `Modulus`.
    pub fn alloc_zero(&self) -> Storage<M> {
        Storage {
            limbs: BoxedLimbs::zero(self.limbs.len()),
        }
    }

    #[inline]
    pub(super) fn limbs(&self) -> &[Limb] {
        self.limbs
    }

    #[inline]
    pub(super) fn n0(&self) -> &N0 {
        &self.n0
    }

    pub fn len_bits(&self) -> BitLength {
        self.len_bits
    }

    #[inline]
    pub(crate) fn cpu_features(&self) -> cpu::Features {
        self.cpu_features
    }
}
