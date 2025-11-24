// Copyright 2017 Brian Smith.
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

use crate::ec::suite_b::ops::{
    p256::NUM_LIMBS as P256_NUM_LIMBS, p384::NUM_LIMBS as P384_NUM_LIMBS,
};
use crate::{
    arithmetic::{
        limbs_from_hex,
        montgomery::{Encoding, ProductEncoding, Unencoded},
    },
    limb::{LeakyLimb, Limb},
};
use core::marker::PhantomData;

#[derive(Clone, Copy)]
pub(super) enum NumLimbs {
    P256,
    P384,
}

impl NumLimbs {
    pub(super) const MAX: usize = Self::P384.into();

    pub(super) const fn into(self) -> usize {
        match self {
            NumLimbs::P256 => P256_NUM_LIMBS,
            NumLimbs::P384 => P384_NUM_LIMBS,
        }
    }
}

/// Elements of ℤ/mℤ for some modulus *m*. Elements are always fully reduced
/// with respect to *m*; i.e. the 0 <= x < m for every value x.
#[derive(Clone, Copy)]
pub struct Elem<M, E: Encoding> {
    // XXX: pub
    pub(super) limbs: [Limb; NumLimbs::MAX],

    /// The modulus *m* for the ring ℤ/mℤ for which this element is a value.
    pub(super) m: PhantomData<M>,

    /// The number of Montgomery factors that need to be canceled out from
    /// `value` to get the actual value.
    pub(super) encoding: PhantomData<E>,
}

pub struct PublicElem<M, E: Encoding> {
    pub(super) limbs: [LeakyLimb; NumLimbs::MAX],
    pub(super) m: PhantomData<M>,
    pub(super) encoding: PhantomData<E>,
}

impl<M, E: Encoding> From<&PublicElem<M, E>> for Elem<M, E> {
    fn from(value: &PublicElem<M, E>) -> Self {
        Self {
            limbs: core::array::from_fn(|i| Limb::from(value.limbs[i])),
            m: value.m,
            encoding: value.encoding,
        }
    }
}

impl<M, E: Encoding> Elem<M, E> {
    // There's no need to convert `value` to the Montgomery domain since
    // 0 * R**2 (mod m) == 0, so neither the modulus nor the encoding are needed
    // as inputs for constructing a zero-valued element.
    pub fn zero() -> Self {
        Self {
            limbs: [0; NumLimbs::MAX],
            m: PhantomData,
            encoding: PhantomData,
        }
    }
}

impl<M> Elem<M, Unencoded> {
    pub fn one() -> Self {
        let mut r = Self::zero();
        r.limbs[0] = 1;
        r
    }
}

impl<M, E: Encoding> PublicElem<M, E> {
    pub const fn from_hex(hex: &str) -> Self {
        Self {
            limbs: limbs_from_hex(hex),
            m: PhantomData,
            encoding: PhantomData,
        }
    }
}

#[inline]
pub fn mul_mont<M, EA: Encoding, EB: Encoding>(
    f: unsafe extern "C" fn(r: *mut Limb, a: *const Limb, b: *const Limb),
    a: &Elem<M, EA>,
    b: &Elem<M, EB>,
) -> Elem<M, <(EA, EB) as ProductEncoding>::Output>
where
    (EA, EB): ProductEncoding,
{
    binary_op(f, a, b)
}

// let r = f(a, b); return r;
#[inline]
pub fn binary_op<M, EA: Encoding, EB: Encoding, ER: Encoding>(
    f: unsafe extern "C" fn(r: *mut Limb, a: *const Limb, b: *const Limb),
    a: &Elem<M, EA>,
    b: &Elem<M, EB>,
) -> Elem<M, ER> {
    let mut r = Elem::zero();
    unsafe { f(r.limbs.as_mut_ptr(), a.limbs.as_ptr(), b.limbs.as_ptr()) }
    r
}

// a := f(a, b);
#[inline]
pub fn binary_op_assign<M, EA: Encoding, EB: Encoding>(
    f: unsafe extern "C" fn(r: *mut Limb, a: *const Limb, b: *const Limb),
    a: &mut Elem<M, EA>,
    b: &Elem<M, EB>,
) {
    unsafe { f(a.limbs.as_mut_ptr(), a.limbs.as_ptr(), b.limbs.as_ptr()) }
}

// let r = f(a); return r;
#[inline]
pub fn unary_op<M, E: Encoding>(
    f: unsafe extern "C" fn(r: *mut Limb, a: *const Limb),
    a: &Elem<M, E>,
) -> Elem<M, E> {
    let mut r = Elem::zero();
    unsafe { f(r.limbs.as_mut_ptr(), a.limbs.as_ptr()) }
    r
}

// a := f(a);
#[inline]
pub fn unary_op_assign<M, E: Encoding>(
    f: unsafe extern "C" fn(r: *mut Limb, a: *const Limb),
    a: &mut Elem<M, E>,
) {
    unsafe { f(a.limbs.as_mut_ptr(), a.limbs.as_ptr()) }
}

// a := f(a, a);
#[inline]
pub fn unary_op_from_binary_op_assign<M, E: Encoding>(
    f: unsafe extern "C" fn(r: *mut Limb, a: *const Limb, b: *const Limb),
    a: &mut Elem<M, E>,
) {
    unsafe { f(a.limbs.as_mut_ptr(), a.limbs.as_ptr(), a.limbs.as_ptr()) }
}
