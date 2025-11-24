// Copyright 2015-2023 Brian Smith.
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

//! Multi-precision integers.
//!
//! # Modular Arithmetic.
//!
//! Modular arithmetic is done in finite commutative rings ℤ/mℤ for some
//! modulus *m*. We work in finite commutative rings instead of finite fields
//! because the RSA public modulus *n* is not prime, which means ℤ/nℤ contains
//! nonzero elements that have no multiplicative inverse, so ℤ/nℤ is not a
//! finite field.
//!
//! In some calculations we need to deal with multiple rings at once. For
//! example, RSA private key operations operate in the rings ℤ/nℤ, ℤ/pℤ, and
//! ℤ/qℤ. Types and functions dealing with such rings are all parameterized
//! over a type `M` to ensure that we don't wrongly mix up the math, e.g. by
//! multiplying an element of ℤ/pℤ by an element of ℤ/qℤ modulo q. This follows
//! the "unit" pattern described in [Static checking of units in Servo].
//!
//! `Elem` also uses the static unit checking pattern to statically track the
//! Montgomery factors that need to be canceled out in each value using it's
//! `E` parameter.
//!
//! [Static checking of units in Servo]:
//!     https://blog.mozilla.org/research/2014/06/23/static-checking-of-units-in-servo/

use self::boxed_limbs::BoxedLimbs;
pub(crate) use self::{
    modulus::{Modulus, OwnedModulus},
    modulusvalue::OwnedModulusValue,
    private_exponent::PrivateExponent,
};
use super::{inout::AliasingSlices3, limbs512, montgomery::*, LimbSliceError, MAX_LIMBS};
use crate::{
    bits::BitLength,
    c,
    error::{self, LenMismatchError},
    limb::{self, Limb, LIMB_BITS},
    polyfill::slice::{self, AsChunks},
};
use core::{
    marker::PhantomData,
    num::{NonZeroU64, NonZeroUsize},
};

mod boxed_limbs;
mod modulus;
mod modulusvalue;
mod private_exponent;

pub trait PublicModulus {}

// When we need to create a new `Elem`, first we create a `Storage` and then
// move its `limbs` into the new element. When we want to recylce an `Elem`'s
// memory allocation, we convert it back into a `Storage`.
pub struct Storage<M> {
    limbs: BoxedLimbs<M>,
}

impl<M, E> From<Elem<M, E>> for Storage<M> {
    fn from(elem: Elem<M, E>) -> Self {
        Self { limbs: elem.limbs }
    }
}

/// Elements of ℤ/mℤ for some modulus *m*.
//
// Defaulting `E` to `Unencoded` is a convenience for callers from outside this
// submodule. However, for maximum clarity, we always explicitly use
// `Unencoded` within the `bigint` submodule.
pub struct Elem<M, E = Unencoded> {
    limbs: BoxedLimbs<M>,

    /// The number of Montgomery factors that need to be canceled out from
    /// `value` to get the actual value.
    encoding: PhantomData<E>,
}

impl<M, E> Elem<M, E> {
    pub fn clone_into(&self, mut out: Storage<M>) -> Self {
        out.limbs.copy_from_slice(&self.limbs);
        Self {
            limbs: out.limbs,
            encoding: self.encoding,
        }
    }
}

impl<M, E> Elem<M, E> {
    #[inline]
    pub fn is_zero(&self) -> bool {
        limb::limbs_are_zero(&self.limbs).leak()
    }
}

/// Does a Montgomery reduction on `limbs` assuming they are Montgomery-encoded ('R') and assuming
/// they are the same size as `m`, but perhaps not reduced mod `m`. The result will be
/// fully reduced mod `m`.
///
/// WARNING: Takes a `Storage` as an in/out value.
fn from_montgomery_amm<M>(mut in_out: Storage<M>, m: &Modulus<M>) -> Elem<M, Unencoded> {
    let mut one = [0; MAX_LIMBS];
    one[0] = 1;
    let one = &one[..m.limbs().len()];
    limbs_mul_mont(
        (&mut in_out.limbs[..], one),
        m.limbs(),
        m.n0(),
        m.cpu_features(),
    )
    .unwrap_or_else(unwrap_impossible_limb_slice_error);
    Elem {
        limbs: in_out.limbs,
        encoding: PhantomData,
    }
}

#[cfg(any(test, not(target_arch = "x86_64")))]
impl<M> Elem<M, R> {
    #[inline]
    pub fn into_unencoded(self, m: &Modulus<M>) -> Elem<M, Unencoded> {
        from_montgomery_amm(Storage::from(self), m)
    }
}

impl<M> Elem<M, Unencoded> {
    pub fn from_be_bytes_padded(
        input: untrusted::Input,
        m: &Modulus<M>,
    ) -> Result<Self, error::Unspecified> {
        Ok(Self {
            limbs: BoxedLimbs::from_be_bytes_padded_less_than(input, m)?,
            encoding: PhantomData,
        })
    }

    #[inline]
    pub fn fill_be_bytes(&self, out: &mut [u8]) {
        // See Falko Strenzke, "Manger's Attack revisited", ICICS 2010.
        limb::big_endian_from_limbs(&self.limbs, out)
    }
}

pub fn elem_mul_into<M, AF, BF>(
    mut out: Storage<M>,
    a: &Elem<M, AF>,
    b: &Elem<M, BF>,
    m: &Modulus<M>,
) -> Elem<M, <(AF, BF) as ProductEncoding>::Output>
where
    (AF, BF): ProductEncoding,
{
    limbs_mul_mont(
        (out.limbs.as_mut(), b.limbs.as_ref(), a.limbs.as_ref()),
        m.limbs(),
        m.n0(),
        m.cpu_features(),
    )
    .unwrap_or_else(unwrap_impossible_limb_slice_error);
    Elem {
        limbs: out.limbs,
        encoding: PhantomData,
    }
}

pub fn elem_mul<M, AF, BF>(
    a: &Elem<M, AF>,
    mut b: Elem<M, BF>,
    m: &Modulus<M>,
) -> Elem<M, <(AF, BF) as ProductEncoding>::Output>
where
    (AF, BF): ProductEncoding,
{
    limbs_mul_mont(
        (&mut b.limbs[..], &a.limbs[..]),
        m.limbs(),
        m.n0(),
        m.cpu_features(),
    )
    .unwrap_or_else(unwrap_impossible_limb_slice_error);
    Elem {
        limbs: b.limbs,
        encoding: PhantomData,
    }
}

// r *= 2.
fn elem_double<M, AF>(r: &mut Elem<M, AF>, m: &Modulus<M>) {
    limb::limbs_double_mod(&mut r.limbs, m.limbs())
        .unwrap_or_else(unwrap_impossible_len_mismatch_error)
}

// TODO: This is currently unused, but we intend to eventually use this to
// reduce elements (x mod q) mod p in the RSA CRT. If/when we do so, we
// should update the testing so it is reflective of that usage, instead of
// the old usage.
pub fn elem_reduced_once<A, M>(
    mut r: Storage<M>,
    a: &Elem<A, Unencoded>,
    m: &Modulus<M>,
    other_modulus_len_bits: BitLength,
) -> Elem<M, Unencoded> {
    assert_eq!(m.len_bits(), other_modulus_len_bits);
    r.limbs.copy_from_slice(&a.limbs);
    limb::limbs_reduce_once(&mut r.limbs, m.limbs())
        .unwrap_or_else(unwrap_impossible_len_mismatch_error);
    Elem {
        limbs: r.limbs,
        encoding: PhantomData,
    }
}

#[inline]
pub fn elem_reduced<Larger, Smaller>(
    mut r: Storage<Smaller>,
    a: &Elem<Larger, Unencoded>,
    m: &Modulus<Smaller>,
    other_prime_len_bits: BitLength,
) -> Elem<Smaller, RInverse> {
    // This is stricter than required mathematically but this is what we
    // guarantee and this is easier to check. The real requirement is that
    // that `a < m*R` where `R` is the Montgomery `R` for `m`.
    assert_eq!(other_prime_len_bits, m.len_bits());

    // `limbs_from_mont_in_place` requires this.
    assert_eq!(a.limbs.len(), m.limbs().len() * 2);

    let mut tmp = [0; MAX_LIMBS];
    let tmp = &mut tmp[..a.limbs.len()];
    tmp.copy_from_slice(&a.limbs);

    limbs_from_mont_in_place(&mut r.limbs, tmp, m.limbs(), m.n0());
    Elem {
        limbs: r.limbs,
        encoding: PhantomData,
    }
}

#[inline]
fn elem_squared<M, E>(
    mut a: Elem<M, E>,
    m: &Modulus<M>,
) -> Elem<M, <(E, E) as ProductEncoding>::Output>
where
    (E, E): ProductEncoding,
{
    limbs_square_mont(&mut a.limbs, m.limbs(), m.n0(), m.cpu_features())
        .unwrap_or_else(unwrap_impossible_limb_slice_error);
    Elem {
        limbs: a.limbs,
        encoding: PhantomData,
    }
}

pub fn elem_widen<Larger, Smaller>(
    mut r: Storage<Larger>,
    a: Elem<Smaller, Unencoded>,
    m: &Modulus<Larger>,
    smaller_modulus_bits: BitLength,
) -> Result<Elem<Larger, Unencoded>, error::Unspecified> {
    if smaller_modulus_bits >= m.len_bits() {
        return Err(error::Unspecified);
    }
    let (to_copy, to_zero) = r.limbs.split_at_mut(a.limbs.len());
    to_copy.copy_from_slice(&a.limbs);
    to_zero.fill(0);
    Ok(Elem {
        limbs: r.limbs,
        encoding: PhantomData,
    })
}

// TODO: Document why this works for all Montgomery factors.
pub fn elem_add<M, E>(mut a: Elem<M, E>, b: Elem<M, E>, m: &Modulus<M>) -> Elem<M, E> {
    limb::limbs_add_assign_mod(&mut a.limbs, &b.limbs, m.limbs())
        .unwrap_or_else(unwrap_impossible_len_mismatch_error);
    a
}

// TODO: Document why this works for all Montgomery factors.
pub fn elem_sub<M, E>(mut a: Elem<M, E>, b: &Elem<M, E>, m: &Modulus<M>) -> Elem<M, E> {
    prefixed_extern! {
        // `r` and `a` may alias.
        fn LIMBS_sub_mod(
            r: *mut Limb,
            a: *const Limb,
            b: *const Limb,
            m: *const Limb,
            num_limbs: c::NonZero_size_t,
        );
    }
    let num_limbs = NonZeroUsize::new(m.limbs().len()).unwrap();
    (a.limbs.as_mut(), b.limbs.as_ref())
        .with_non_dangling_non_null_pointers_rab(num_limbs, |r, a, b| {
            let m = m.limbs().as_ptr(); // Also non-dangling because num_limbs is non-zero.
            unsafe { LIMBS_sub_mod(r, a, b, m, num_limbs) }
        })
        .unwrap_or_else(unwrap_impossible_len_mismatch_error);
    a
}

// The value 1, Montgomery-encoded some number of times.
pub struct One<M, E>(Elem<M, E>);

impl<M> One<M, RR> {
    // Returns RR = = R**2 (mod n) where R = 2**r is the smallest power of
    // 2**LIMB_BITS such that R > m.
    //
    // Even though the assembly on some 32-bit platforms works with 64-bit
    // values, using `LIMB_BITS` here, rather than `N0::LIMBS_USED * LIMB_BITS`,
    // is correct because R**2 will still be a multiple of the latter as
    // `N0::LIMBS_USED` is either one or two.
    pub(crate) fn newRR(mut out: Storage<M>, m: &Modulus<M>) -> Self {
        // The number of limbs in the numbers involved.
        let w = m.limbs().len();

        // The length of the numbers involved, in bits. R = 2**r.
        let r = w * LIMB_BITS;

        m.oneR(&mut out.limbs);
        let mut acc: Elem<M, R> = Elem {
            limbs: out.limbs,
            encoding: PhantomData,
        };

        // 2**t * R can be calculated by t doublings starting with R.
        //
        // Choose a t that divides r and where t doublings are cheaper than 1 squaring.
        //
        // We could choose other values of t than w. But if t < d then the exponentiation that
        // follows would require multiplications. Normally d is 1 (i.e. the modulus length is a
        // power of two: RSA 1024, 2048, 4097, 8192) or 3 (RSA 1536, 3072).
        //
        // XXX(perf): Currently t = w / 2 is slightly faster. TODO(perf): Optimize `elem_double`
        // and re-run benchmarks to rebalance this.
        let t = w;
        let z = w.trailing_zeros();
        let d = w >> z;
        debug_assert_eq!(w, d * (1 << z));
        debug_assert!(d <= t);
        debug_assert!(t < r);
        for _ in 0..t {
            elem_double(&mut acc, m);
        }

        // Because t | r:
        //
        //     MontExp(2**t * R, r / t)
        //   = (2**t)**(r / t)   * R (mod m) by definition of MontExp.
        //   = (2**t)**(1/t * r) * R (mod m)
        //   = (2**(t * 1/t))**r * R (mod m)
        //   = (2**1)**r         * R (mod m)
        //   = 2**r              * R (mod m)
        //   = R * R                 (mod m)
        //   = RR
        //
        // Like BoringSSL, use t = w (`m.limbs.len()`) which ensures that the exponent is a power
        // of two. Consequently, there will be no multiplications in the Montgomery exponentiation;
        // there will only be lg(r / t) squarings.
        //
        //     lg(r / t)
        //   = lg((w * 2**b) / t)
        //   = lg((t * 2**b) / t)
        //   = lg(2**b)
        //   = b
        // TODO(MSRV:1.67): const B: u32 = LIMB_BITS.ilog2();
        const B: u32 = if cfg!(target_pointer_width = "64") {
            6
        } else if cfg!(target_pointer_width = "32") {
            5
        } else {
            panic!("unsupported target_pointer_width")
        };
        #[allow(clippy::assertions_on_constants)]
        const _LIMB_BITS_IS_2_POW_B: () = assert!(LIMB_BITS == 1 << B);
        debug_assert_eq!(r, t * (1 << B));
        for _ in 0..B {
            acc = elem_squared(acc, m);
        }

        Self(Elem {
            limbs: acc.limbs,
            encoding: PhantomData, // PhantomData<RR>
        })
    }
}

impl<M> One<M, RRR> {
    pub(crate) fn newRRR(One(oneRR): One<M, RR>, m: &Modulus<M>) -> Self {
        Self(elem_squared(oneRR, m))
    }
}

impl<M, E> AsRef<Elem<M, E>> for One<M, E> {
    fn as_ref(&self) -> &Elem<M, E> {
        &self.0
    }
}

impl<M: PublicModulus, E> One<M, E> {
    pub fn clone_into(&self, out: Storage<M>) -> Self {
        Self(self.0.clone_into(out))
    }
}

/// Calculates base**exponent (mod m).
///
/// The run time  is a function of the number of limbs in `m` and the bit
/// length and Hamming Weight of `exponent`. The bounds on `m` are pretty
/// obvious but the bounds on `exponent` are less obvious. Callers should
/// document the bounds they place on the maximum value and maximum Hamming
/// weight of `exponent`.
// TODO: The test coverage needs to be expanded, e.g. test with the largest
// accepted exponent and with the most common values of 65537 and 3.
pub(crate) fn elem_exp_vartime<M>(
    out: Storage<M>,
    base: Elem<M, R>,
    exponent: NonZeroU64,
    m: &Modulus<M>,
) -> Elem<M, R> {
    // Use what [Knuth] calls the "S-and-X binary method", i.e. variable-time
    // square-and-multiply that scans the exponent from the most significant
    // bit to the least significant bit (left-to-right). Left-to-right requires
    // less storage compared to right-to-left scanning, at the cost of needing
    // to compute `exponent.leading_zeros()`, which we assume to be cheap.
    //
    // As explained in [Knuth], exponentiation by squaring is the most
    // efficient algorithm when the Hamming weight is 2 or less. It isn't the
    // most efficient for all other, uncommon, exponent values but any
    // suboptimality is bounded at least by the small bit length of `exponent`
    // as enforced by its type.
    //
    // This implementation is slightly simplified by taking advantage of the
    // fact that we require the exponent to be a positive integer.
    //
    // [Knuth]: The Art of Computer Programming, Volume 2: Seminumerical
    //          Algorithms (3rd Edition), Section 4.6.3.
    let exponent = exponent.get();
    let mut acc = base.clone_into(out);
    let mut bit = 1 << (64 - 1 - exponent.leading_zeros());
    debug_assert!((exponent & bit) != 0);
    while bit > 1 {
        bit >>= 1;
        acc = elem_squared(acc, m);
        if (exponent & bit) != 0 {
            acc = elem_mul(&base, acc, m);
        }
    }
    acc
}

pub fn elem_exp_consttime<N, P>(
    out: Storage<P>,
    base: &Elem<N>,
    oneRRR: &One<P, RRR>,
    exponent: &PrivateExponent,
    p: &Modulus<P>,
    other_prime_len_bits: BitLength,
) -> Result<Elem<P, Unencoded>, LimbSliceError> {
    // `elem_exp_consttime_inner` is parameterized on `STORAGE_LIMBS` only so
    // we can run tests with larger-than-supported-in-operation test vectors.
    elem_exp_consttime_inner::<N, P, { ELEM_EXP_CONSTTIME_MAX_MODULUS_LIMBS * STORAGE_ENTRIES }>(
        out,
        base,
        oneRRR,
        exponent,
        p,
        other_prime_len_bits,
    )
}

// The maximum modulus size supported for `elem_exp_consttime` in normal
// operation.
const ELEM_EXP_CONSTTIME_MAX_MODULUS_LIMBS: usize = 2048 / LIMB_BITS;
const _LIMBS_PER_CHUNK_DIVIDES_ELEM_EXP_CONSTTIME_MAX_MODULUS_LIMBS: () =
    assert!(ELEM_EXP_CONSTTIME_MAX_MODULUS_LIMBS % limbs512::LIMBS_PER_CHUNK == 0);
const WINDOW_BITS: u32 = 5;
const TABLE_ENTRIES: usize = 1 << WINDOW_BITS;
const STORAGE_ENTRIES: usize = TABLE_ENTRIES + if cfg!(target_arch = "x86_64") { 3 } else { 0 };

#[cfg(not(target_arch = "x86_64"))]
fn elem_exp_consttime_inner<N, M, const STORAGE_LIMBS: usize>(
    out: Storage<M>,
    base_mod_n: &Elem<N>,
    oneRRR: &One<M, RRR>,
    exponent: &PrivateExponent,
    m: &Modulus<M>,
    other_prime_len_bits: BitLength,
) -> Result<Elem<M, Unencoded>, LimbSliceError> {
    use crate::{bssl, limb::Window};

    let base_rinverse: Elem<M, RInverse> = elem_reduced(out, base_mod_n, m, other_prime_len_bits);

    let num_limbs = m.limbs().len();
    let m_chunked: AsChunks<Limb, { limbs512::LIMBS_PER_CHUNK }> = match slice::as_chunks(m.limbs())
    {
        (m, []) => m,
        _ => {
            return Err(LimbSliceError::len_mismatch(LenMismatchError::new(
                num_limbs,
            )))
        }
    };
    let cpe = m_chunked.len(); // 512-bit chunks per entry.

    // This code doesn't have the strict alignment requirements that the x86_64
    // version does, but uses the same aligned storage for convenience.
    assert!(STORAGE_LIMBS % (STORAGE_ENTRIES * limbs512::LIMBS_PER_CHUNK) == 0); // TODO: `const`
    let mut table = limbs512::AlignedStorage::<STORAGE_LIMBS>::zeroed();
    let mut table = table
        .aligned_chunks_mut(TABLE_ENTRIES, cpe)
        .map_err(LimbSliceError::len_mismatch)?;

    // TODO: Rewrite the below in terms of `AsChunks`.
    let table = table.as_flattened_mut();

    fn gather<M>(table: &[Limb], acc: &mut Elem<M, R>, i: Window) {
        prefixed_extern! {
            fn LIMBS_select_512_32(
                r: *mut Limb,
                table: *const Limb,
                num_limbs: c::size_t,
                i: Window,
            ) -> bssl::Result;
        }
        Result::from(unsafe {
            LIMBS_select_512_32(acc.limbs.as_mut_ptr(), table.as_ptr(), acc.limbs.len(), i)
        })
        .unwrap();
    }

    fn power<M>(
        table: &[Limb],
        mut acc: Elem<M, R>,
        m: &Modulus<M>,
        i: Window,
        mut tmp: Elem<M, R>,
    ) -> (Elem<M, R>, Elem<M, R>) {
        for _ in 0..WINDOW_BITS {
            acc = elem_squared(acc, m);
        }
        gather(table, &mut tmp, i);
        let acc = elem_mul(&tmp, acc, m);
        (acc, tmp)
    }

    fn entry(table: &[Limb], i: usize, num_limbs: usize) -> &[Limb] {
        &table[(i * num_limbs)..][..num_limbs]
    }
    fn entry_mut(table: &mut [Limb], i: usize, num_limbs: usize) -> &mut [Limb] {
        &mut table[(i * num_limbs)..][..num_limbs]
    }

    // table[0] = base**0 (i.e. 1).
    m.oneR(entry_mut(table, 0, num_limbs));

    // table[1] = base*R == (base/R * RRR)/R
    limbs_mul_mont(
        (
            entry_mut(table, 1, num_limbs),
            base_rinverse.limbs.as_ref(),
            oneRRR.as_ref().limbs.as_ref(),
        ),
        m.limbs(),
        m.n0(),
        m.cpu_features(),
    )?;
    for i in 2..TABLE_ENTRIES {
        let (src1, src2) = if i % 2 == 0 {
            (i / 2, i / 2)
        } else {
            (i - 1, 1)
        };
        let (previous, rest) = table.split_at_mut(num_limbs * i);
        let src1 = entry(previous, src1, num_limbs);
        let src2 = entry(previous, src2, num_limbs);
        let dst = entry_mut(rest, 0, num_limbs);
        limbs_mul_mont((dst, src1, src2), m.limbs(), m.n0(), m.cpu_features())?;
    }

    let mut acc = Elem {
        limbs: base_rinverse.limbs,
        encoding: PhantomData,
    };
    let tmp = m.alloc_zero();
    let tmp = Elem {
        limbs: tmp.limbs,
        encoding: PhantomData,
    };
    let (acc, _) = limb::fold_5_bit_windows(
        exponent.limbs(),
        |initial_window| {
            gather(&table, &mut acc, initial_window);
            (acc, tmp)
        },
        |(acc, tmp), window| power(&table, acc, m, window, tmp),
    );

    Ok(acc.into_unencoded(m))
}

#[cfg(target_arch = "x86_64")]
fn elem_exp_consttime_inner<N, M, const STORAGE_LIMBS: usize>(
    out: Storage<M>,
    base_mod_n: &Elem<N>,
    oneRRR: &One<M, RRR>,
    exponent: &PrivateExponent,
    m: &Modulus<M>,
    other_prime_len_bits: BitLength,
) -> Result<Elem<M, Unencoded>, LimbSliceError> {
    use super::limbs::x86_64::mont::{
        gather5, mul_mont5, mul_mont_gather5_amm, power5_amm, scatter5, sqr_mont5,
    };
    use crate::{
        cpu::{
            intel::{Adx, Bmi2},
            GetFeature as _,
        },
        limb::{LeakyWindow, Window},
        polyfill::slice::AsChunksMut,
    };

    let n0 = m.n0();

    let cpu2 = m.cpu_features().get_feature();
    let cpu3 = m.cpu_features().get_feature();

    if base_mod_n.limbs.len() != m.limbs().len() * 2 {
        return Err(LimbSliceError::len_mismatch(LenMismatchError::new(
            base_mod_n.limbs.len(),
        )));
    }

    let m_original: AsChunks<Limb, 8> = match slice::as_chunks(m.limbs()) {
        (m, []) => m,
        _ => return Err(LimbSliceError::len_mismatch(LenMismatchError::new(8))),
    };
    let cpe = m_original.len(); // 512-bit chunks per entry

    let oneRRR = &oneRRR.as_ref().limbs;
    let oneRRR = match slice::as_chunks(oneRRR) {
        (c, []) => c,
        _ => {
            return Err(LimbSliceError::len_mismatch(LenMismatchError::new(
                oneRRR.len(),
            )))
        }
    };

    // The x86_64 assembly was written under the assumption that the input data
    // is aligned to `MOD_EXP_CTIME_ALIGN` bytes, which was/is 64 in OpenSSL.
    // Subsequently, it was changed such that, according to BoringSSL, they
    // only require 16 byte alignment. We enforce the old, stronger, alignment
    // unless/until we can see a benefit to reducing it.
    //
    // Similarly, OpenSSL uses the x86_64 assembly functions by giving it only
    // inputs `tmp`, `am`, and `np` that immediately follow the table.
    // According to BoringSSL, in older versions of the OpenSSL code, this
    // extra space was required for memory safety because the assembly code
    // would over-read the table; according to BoringSSL, this is no longer the
    // case. Regardless, the upstream code also contained comments implying
    // that this was also important for performance. For now, we do as OpenSSL
    // did/does.
    const MOD_EXP_CTIME_ALIGN: usize = 64;
    // Required by
    const _TABLE_ENTRIES_IS_32: () = assert!(TABLE_ENTRIES == 32);
    const _STORAGE_ENTRIES_HAS_3_EXTRA: () = assert!(STORAGE_ENTRIES == TABLE_ENTRIES + 3);

    assert!(STORAGE_LIMBS % (STORAGE_ENTRIES * limbs512::LIMBS_PER_CHUNK) == 0); // TODO: `const`
    let mut table = limbs512::AlignedStorage::<STORAGE_LIMBS>::zeroed();
    let mut table = table
        .aligned_chunks_mut(STORAGE_ENTRIES, cpe)
        .map_err(LimbSliceError::len_mismatch)?;
    let (mut table, mut state) = table.split_at_mut(TABLE_ENTRIES * cpe);
    assert_eq!((table.as_ptr() as usize) % MOD_EXP_CTIME_ALIGN, 0);

    // These are named `(tmp, am, np)` in BoringSSL.
    let (mut acc, mut rest) = state.split_at_mut(cpe);
    let (mut base_cached, mut m_cached) = rest.split_at_mut(cpe);

    // "To improve cache locality" according to upstream.
    m_cached
        .as_flattened_mut()
        .copy_from_slice(m_original.as_flattened());
    let m_cached = m_cached.as_ref();

    let out: Elem<M, RInverse> = elem_reduced(out, base_mod_n, m, other_prime_len_bits);
    let base_rinverse = match slice::as_chunks(&out.limbs) {
        (c, []) => c,
        _ => {
            return Err(LimbSliceError::len_mismatch(LenMismatchError::new(
                out.limbs.len(),
            )))
        }
    };

    // base_cached = base*R == (base/R * RRR)/R
    mul_mont5(
        base_cached.as_mut(),
        base_rinverse,
        oneRRR,
        m_cached,
        n0,
        cpu2,
    )?;
    let base_cached = base_cached.as_ref();
    let mut out = Storage::from(out); // recycle.

    // Fill in all the powers of 2 of `acc` into the table using only squaring and without any
    // gathering, storing the last calculated power into `acc`.
    fn scatter_powers_of_2(
        mut table: AsChunksMut<Limb, 8>,
        mut acc: AsChunksMut<Limb, 8>,
        m_cached: AsChunks<Limb, 8>,
        n0: &N0,
        mut i: LeakyWindow,
        cpu: Option<(Adx, Bmi2)>,
    ) -> Result<(), LimbSliceError> {
        loop {
            scatter5(acc.as_ref(), table.as_mut(), i)?;
            i *= 2;
            if i >= TABLE_ENTRIES as LeakyWindow {
                break;
            }
            sqr_mont5(acc.as_mut(), m_cached, n0, cpu)?;
        }
        Ok(())
    }

    // All entries in `table` will be Montgomery encoded.

    // acc = table[0] = base**0 (i.e. 1).
    m.oneR(acc.as_flattened_mut());
    scatter5(acc.as_ref(), table.as_mut(), 0)?;

    // acc = base**1 (i.e. base).
    acc.as_flattened_mut()
        .copy_from_slice(base_cached.as_flattened());

    // Fill in entries 1, 2, 4, 8, 16.
    scatter_powers_of_2(table.as_mut(), acc.as_mut(), m_cached, n0, 1, cpu2)?;
    // Fill in entries 3, 6, 12, 24; 5, 10, 20, 30; 7, 14, 28; 9, 18; 11, 22; 13, 26; 15, 30;
    // 17; 19; 21; 23; 25; 27; 29; 31.
    for i in (3..(TABLE_ENTRIES as LeakyWindow)).step_by(2) {
        let power = Window::from(i - 1);
        assert!(power < 32); // Not secret,
        unsafe {
            mul_mont_gather5_amm(
                acc.as_mut(),
                base_cached,
                table.as_ref(),
                m_cached,
                n0,
                power,
                cpu3,
            )
        }?;
        scatter_powers_of_2(table.as_mut(), acc.as_mut(), m_cached, n0, i, cpu2)?;
    }

    let table = table.as_ref();

    let acc = limb::fold_5_bit_windows(
        exponent.limbs(),
        |initial_window| {
            unsafe { gather5(acc.as_mut(), table, initial_window) }
                .unwrap_or_else(unwrap_impossible_limb_slice_error);
            acc
        },
        |mut acc, window| {
            unsafe { power5_amm(acc.as_mut(), table, m_cached, n0, window, cpu3) }
                .unwrap_or_else(unwrap_impossible_limb_slice_error);
            acc
        },
    );

    // Reuse `base_rinverse`'s limbs to save an allocation.
    out.limbs.copy_from_slice(acc.as_flattened());
    Ok(from_montgomery_amm(out, m))
}

/// Verified a == b**-1 (mod m), i.e. a**-1 == b (mod m).
pub fn verify_inverses_consttime<M>(
    a: &Elem<M, R>,
    b: Elem<M, Unencoded>,
    m: &Modulus<M>,
) -> Result<(), error::Unspecified> {
    let r = elem_mul(a, b, m);
    limb::verify_limbs_equal_1_leak_bit(&r.limbs)
}

#[inline]
pub fn elem_verify_equal_consttime<M, E>(
    a: &Elem<M, E>,
    b: &Elem<M, E>,
) -> Result<(), error::Unspecified> {
    let equal = limb::limbs_equal_limbs_consttime(&a.limbs, &b.limbs)
        .unwrap_or_else(unwrap_impossible_len_mismatch_error);
    if !equal.leak() {
        return Err(error::Unspecified);
    }
    Ok(())
}

#[cold]
#[inline(never)]
fn unwrap_impossible_len_mismatch_error<T>(LenMismatchError { .. }: LenMismatchError) -> T {
    unreachable!()
}

#[cold]
#[inline(never)]
fn unwrap_impossible_limb_slice_error(err: LimbSliceError) {
    match err {
        LimbSliceError::LenMismatch(_) => unreachable!(),
        LimbSliceError::TooShort(_) => unreachable!(),
        LimbSliceError::TooLong(_) => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu;
    use crate::testutil as test;

    // Type-level representation of an arbitrary modulus.
    struct M {}

    impl PublicModulus for M {}

    #[test]
    fn test_elem_exp_consttime() {
        let cpu_features = cpu::features();
        test::run(
            test_vector_file!("../../crypto/fipsmodule/bn/test/mod_exp_tests.txt"),
            |section, test_case| {
                assert_eq!(section, "");

                let m = consume_modulus::<M>(test_case, "M");
                let m = m.modulus(cpu_features);
                let expected_result = consume_elem(test_case, "ModExp", &m);
                let base = consume_elem(test_case, "A", &m);
                let e = {
                    let bytes = test_case.consume_bytes("E");
                    PrivateExponent::from_be_bytes_for_test_only(untrusted::Input::from(&bytes), &m)
                        .expect("valid exponent")
                };

                let oneRR = One::newRR(m.alloc_zero(), &m);
                let oneRRR = One::newRRR(oneRR, &m);

                // `base` in the test vectors is reduced (mod M) already but
                // the API expects the bsae to be (mod N) where N = M * P for
                // some other prime of the same length. Fake that here.
                // Pretend there's another prime of equal length.
                struct N {}
                let other_modulus_len_bits = m.len_bits();
                let base: Elem<N> = {
                    let mut limbs = BoxedLimbs::zero(base.limbs.len() * 2);
                    limbs[..base.limbs.len()].copy_from_slice(&base.limbs);
                    Elem {
                        limbs,
                        encoding: PhantomData,
                    }
                };

                let too_big = m.limbs().len() > ELEM_EXP_CONSTTIME_MAX_MODULUS_LIMBS;
                let actual_result = if !too_big {
                    elem_exp_consttime(
                        m.alloc_zero(),
                        &base,
                        &oneRRR,
                        &e,
                        &m,
                        other_modulus_len_bits,
                    )
                } else {
                    let actual_result = elem_exp_consttime(
                        m.alloc_zero(),
                        &base,
                        &oneRRR,
                        &e,
                        &m,
                        other_modulus_len_bits,
                    );
                    // TODO: Be more specific with which error we expect?
                    assert!(actual_result.is_err());
                    // Try again with a larger-than-normally-supported limit
                    elem_exp_consttime_inner::<_, _, { (4096 / LIMB_BITS) * STORAGE_ENTRIES }>(
                        m.alloc_zero(),
                        &base,
                        &oneRRR,
                        &e,
                        &m,
                        other_modulus_len_bits,
                    )
                };
                match actual_result {
                    Ok(r) => assert_elem_eq(&r, &expected_result),
                    Err(LimbSliceError::LenMismatch { .. }) => panic!(),
                    Err(LimbSliceError::TooLong { .. }) => panic!(),
                    Err(LimbSliceError::TooShort { .. }) => panic!(),
                };

                Ok(())
            },
        )
    }

    // TODO: fn test_elem_exp_vartime() using
    // "src/rsa/bigint_elem_exp_vartime_tests.txt". See that file for details.
    // In the meantime, the function is tested indirectly via the RSA
    // verification and signing tests.
    #[test]
    fn test_elem_mul() {
        let cpu_features = cpu::features();
        test::run(
            test_vector_file!("../../crypto/fipsmodule/bn/test/mod_mul_tests.txt"),
            |section, test_case| {
                assert_eq!(section, "");

                let m = consume_modulus::<M>(test_case, "M");
                let m = m.modulus(cpu_features);
                let expected_result = consume_elem(test_case, "ModMul", &m);
                let a = consume_elem(test_case, "A", &m);
                let b = consume_elem(test_case, "B", &m);

                let b = into_encoded(m.alloc_zero(), b, &m);
                let a = into_encoded(m.alloc_zero(), a, &m);
                let actual_result = elem_mul(&a, b, &m);
                let actual_result = actual_result.into_unencoded(&m);
                assert_elem_eq(&actual_result, &expected_result);

                Ok(())
            },
        )
    }

    #[test]
    fn test_elem_squared() {
        let cpu_features = cpu::features();
        test::run(
            test_vector_file!("bigint_elem_squared_tests.txt"),
            |section, test_case| {
                assert_eq!(section, "");

                let m = consume_modulus::<M>(test_case, "M");
                let m = m.modulus(cpu_features);
                let expected_result = consume_elem(test_case, "ModSquare", &m);
                let a = consume_elem(test_case, "A", &m);

                let a = into_encoded(m.alloc_zero(), a, &m);
                let actual_result = elem_squared(a, &m);
                let actual_result = actual_result.into_unencoded(&m);
                assert_elem_eq(&actual_result, &expected_result);

                Ok(())
            },
        )
    }

    #[test]
    fn test_elem_reduced() {
        let cpu_features = cpu::features();
        test::run(
            test_vector_file!("bigint_elem_reduced_tests.txt"),
            |section, test_case| {
                assert_eq!(section, "");

                struct M {}

                let m_ = consume_modulus::<M>(test_case, "M");
                let m = m_.modulus(cpu_features);
                let expected_result = consume_elem(test_case, "R", &m);
                let a =
                    consume_elem_unchecked::<M>(test_case, "A", expected_result.limbs.len() * 2);
                let other_modulus_len_bits = m_.len_bits();

                let actual_result = elem_reduced(m.alloc_zero(), &a, &m, other_modulus_len_bits);
                let oneRR = One::newRR(m.alloc_zero(), &m);
                let actual_result = elem_mul(oneRR.as_ref(), actual_result, &m);
                assert_elem_eq(&actual_result, &expected_result);

                Ok(())
            },
        )
    }

    #[test]
    fn test_elem_reduced_once() {
        let cpu_features = cpu::features();
        test::run(
            test_vector_file!("bigint_elem_reduced_once_tests.txt"),
            |section, test_case| {
                assert_eq!(section, "");

                struct M {}
                struct O {}
                let m = consume_modulus::<M>(test_case, "m");
                let m = m.modulus(cpu_features);
                let a = consume_elem_unchecked::<O>(test_case, "a", m.limbs().len());
                let expected_result = consume_elem::<M>(test_case, "r", &m);
                let other_modulus_len_bits = m.len_bits();

                let actual_result =
                    elem_reduced_once(m.alloc_zero(), &a, &m, other_modulus_len_bits);
                assert_elem_eq(&actual_result, &expected_result);

                Ok(())
            },
        )
    }

    fn consume_elem<M>(
        test_case: &mut test::TestCase,
        name: &str,
        m: &Modulus<M>,
    ) -> Elem<M, Unencoded> {
        let value = test_case.consume_bytes(name);
        Elem::from_be_bytes_padded(untrusted::Input::from(&value), m).unwrap()
    }

    fn consume_elem_unchecked<M>(
        test_case: &mut test::TestCase,
        name: &str,
        num_limbs: usize,
    ) -> Elem<M, Unencoded> {
        let bytes = test_case.consume_bytes(name);
        let mut limbs = BoxedLimbs::zero(num_limbs);
        limb::parse_big_endian_and_pad_consttime(untrusted::Input::from(&bytes), &mut limbs)
            .unwrap();
        Elem {
            limbs,
            encoding: PhantomData,
        }
    }

    fn consume_modulus<M>(test_case: &mut test::TestCase, name: &str) -> OwnedModulus<M> {
        let value = test_case.consume_bytes(name);
        OwnedModulus::from(
            OwnedModulusValue::from_be_bytes(untrusted::Input::from(&value)).unwrap(),
        )
    }

    fn assert_elem_eq<M, E>(a: &Elem<M, E>, b: &Elem<M, E>) {
        if elem_verify_equal_consttime(a, b).is_err() {
            panic!("{:x?} != {:x?}", &*a.limbs, &*b.limbs);
        }
    }

    fn into_encoded<M>(out: Storage<M>, a: Elem<M, Unencoded>, m: &Modulus<M>) -> Elem<M, R> {
        let oneRR = One::newRR(out, m);
        elem_mul(oneRR.as_ref(), a, m)
    }
}
