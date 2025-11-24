// Copyright 2017-2025 Brian Smith.
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

pub use super::n0::N0;
use super::{inout::AliasingSlices3, LimbSliceError, MIN_LIMBS};
use crate::cpu;
use cfg_if::cfg_if;

// Indicates that the element is not encoded; there is no *R* factor
// that needs to be canceled out.
#[derive(Copy, Clone)]
pub enum Unencoded {}

// Indicates that the element is encoded; the value has one *R*
// factor that needs to be canceled out.
#[derive(Copy, Clone)]
pub enum R {}

// Indicates the element is encoded three times; the value has three
// *R* factors that need to be canceled out.
#[allow(clippy::upper_case_acronyms)]
#[derive(Copy, Clone)]
pub enum RRR {}

// Indicates the element is encoded twice; the value has two *R*
// factors that need to be canceled out.
#[derive(Copy, Clone)]
pub enum RR {}

// Indicates the element is inversely encoded; the value has one
// 1/*R* factor that needs to be canceled out.
#[derive(Copy, Clone)]
pub enum RInverse {}

pub trait Encoding {}

impl Encoding for RRR {}
impl Encoding for RR {}
impl Encoding for R {}
impl Encoding for Unencoded {}
impl Encoding for RInverse {}

/// The encoding of the result of a reduction.
pub trait ReductionEncoding {
    type Output: Encoding;
}

impl ReductionEncoding for RRR {
    type Output = RR;
}

impl ReductionEncoding for RR {
    type Output = R;
}
impl ReductionEncoding for R {
    type Output = Unencoded;
}
impl ReductionEncoding for Unencoded {
    type Output = RInverse;
}

/// The encoding of the result of a multiplication.
pub trait ProductEncoding {
    type Output: Encoding;
}

impl<E: ReductionEncoding> ProductEncoding for (Unencoded, E) {
    type Output = E::Output;
}

impl<E: Encoding> ProductEncoding for (R, E) {
    type Output = E;
}

impl ProductEncoding for (RR, RR) {
    type Output = RRR;
}

impl<E: ReductionEncoding> ProductEncoding for (RInverse, E)
where
    E::Output: ReductionEncoding,
{
    type Output = <<E as ReductionEncoding>::Output as ReductionEncoding>::Output;
}

// XXX: Rust doesn't allow overlapping impls,
// TODO (if/when Rust allows it):
// impl<E1, E2: ReductionEncoding> ProductEncoding for
//         (E1, E2) {
//     type Output = <(E2, E1) as ProductEncoding>::Output;
// }
impl ProductEncoding for (RR, Unencoded) {
    type Output = <(Unencoded, RR) as ProductEncoding>::Output;
}
impl ProductEncoding for (RR, RInverse) {
    type Output = <(RInverse, RR) as ProductEncoding>::Output;
}

impl ProductEncoding for (RRR, RInverse) {
    type Output = <(RInverse, RRR) as ProductEncoding>::Output;
}

#[allow(unused_imports)]
use crate::{bssl, c, limb::Limb};

#[inline(always)]
pub(super) fn limbs_mul_mont(
    in_out: impl AliasingSlices3<Limb>,
    n: &[Limb],
    n0: &N0,
    cpu: cpu::Features,
) -> Result<(), LimbSliceError> {
    const MOD_FALLBACK: usize = 1; // No restriction.
    cfg_if! {
        if #[cfg(all(target_arch = "aarch64", target_endian = "little"))] {
            let _: cpu::Features = cpu;
            const MIN_4X: usize = 4;
            const MOD_4X: usize = 4;
            if n.len() >= MIN_4X && n.len() % MOD_4X == 0 {
                bn_mul_mont_ffi!(in_out, n, n0, (), unsafe {
                    (MIN_4X, MOD_4X, ()) => bn_mul4x_mont
                })
            } else {
                bn_mul_mont_ffi!(in_out, n, n0, (), unsafe {
                    (MIN_LIMBS, MOD_FALLBACK, ()) => bn_mul_mont_nohw
                })
            }
        } else if #[cfg(all(target_arch = "arm", target_endian = "little"))] {
            const MIN_8X: usize = 8;
            const MOD_8X: usize = 8;
            if n.len() >= MIN_8X && n.len() % MOD_8X == 0 {
                use crate::cpu::{GetFeature as _, arm::Neon};
                if let Some(cpu) = cpu.get_feature() {
                    return bn_mul_mont_ffi!(in_out, n, n0, cpu, unsafe {
                        (MIN_8X, MOD_8X, Neon) => bn_mul8x_mont_neon
                    });
                }
            }
            // The ARM version of `bn_mul_mont_nohw` has a minimum of 2.
            const _MIN_LIMBS_AT_LEAST_2: () = assert!(MIN_LIMBS >= 2);
            bn_mul_mont_ffi!(in_out, n, n0, (), unsafe {
                (MIN_LIMBS, MOD_FALLBACK, ()) => bn_mul_mont_nohw
            })
        } else if #[cfg(target_arch = "x86")] {
            use crate::{cpu::GetFeature as _, cpu::intel::Sse2};
            // The X86 implementation of `bn_mul_mont` has a minimum of 4.
            const _MIN_LIMBS_AT_LEAST_4: () = assert!(MIN_LIMBS >= 4);
            if let Some(cpu) = cpu.get_feature() {
                bn_mul_mont_ffi!(in_out, n, n0, cpu, unsafe {
                    (MIN_LIMBS, MOD_FALLBACK, Sse2) => bn_mul_mont
                })
            } else {
                // This isn't really an FFI call; it's defined below.
                unsafe {
                    super::ffi::bn_mul_mont_ffi::<(), {MIN_LIMBS}, 1>(in_out, n, n0, (),
                    bn_mul_mont_fallback)
                }
            }
        } else if #[cfg(target_arch = "x86_64")] {
            use crate::{cpu::GetFeature as _, polyfill::slice};
            use super::limbs::x86_64;
            if n.len() >= x86_64::mont::MIN_4X {
                if let (n, []) = slice::as_chunks(n) {
                    return x86_64::mont::mul_mont5_4x(in_out, n, n0, cpu.get_feature());
                }
            }
            bn_mul_mont_ffi!(in_out, n, n0, (), unsafe {
                (MIN_LIMBS, MOD_FALLBACK, ()) => bn_mul_mont_nohw
            })
        } else {
            // Use the fallback implementation implemented below through the
            // FFI wrapper defined below, so that Rust and C code both go
            // through `bn_mul_mont`.
            bn_mul_mont_ffi!(in_out, n, n0, cpu, unsafe {
                (MIN_LIMBS, MOD_FALLBACK, cpu::Features) => bn_mul_mont
            })
        }
    }
}

cfg_if! {
    if  #[cfg(not(any(
            all(target_arch = "aarch64", target_endian = "little"),
            all(target_arch = "arm", target_endian = "little"),
            target_arch = "x86_64")))] {

        // TODO: Stop calling this from C and un-export it.
        #[cfg(not(target_arch = "x86"))]
        prefixed_export! {
            unsafe extern "C" fn bn_mul_mont(
                r: *mut Limb,
                a: *const Limb,
                b: *const Limb,
                n: *const Limb,
                n0: &N0,
                num_limbs: c::NonZero_size_t,
            ) {
                unsafe { bn_mul_mont_fallback(r, a, b, n, n0, num_limbs) }
            }
        }

        #[cfg_attr(target_arch = "x86", cold)]
        #[cfg_attr(target_arch = "x86", inline(never))]
        unsafe extern "C" fn bn_mul_mont_fallback(
            r: *mut Limb,
            a: *const Limb,
            b: *const Limb,
            n: *const Limb,
            n0: &N0,
            num_limbs: c::NonZero_size_t,
        ) {
            use super::MAX_LIMBS;

            let num_limbs = num_limbs.get();

            // The mutable pointer `r` may alias `a` and/or `b`, so the lifetimes of
            // any slices for `a` or `b` must not overlap with the lifetime of any
            // mutable for `r`.

            // Nothing aliases `n`
            let n = unsafe { core::slice::from_raw_parts(n, num_limbs) };

            let mut tmp = [0; 2 * MAX_LIMBS];
            let tmp = &mut tmp[..(2 * num_limbs)];
            {
                let a: &[Limb] = unsafe { core::slice::from_raw_parts(a, num_limbs) };
                let b: &[Limb] = unsafe { core::slice::from_raw_parts(b, num_limbs) };
                limbs_mul(tmp, a, b);
            }
            let r: &mut [Limb] = unsafe { core::slice::from_raw_parts_mut(r, num_limbs) };
            limbs_from_mont_in_place(r, tmp, n, n0);
        }
    }
}

// `bigint` needs then when the `alloc` feature is enabled. `bn_mul_mont` above needs this when
// we are using the platforms for which we don't have `bn_mul_mont` in assembly.
#[cfg(any(
    feature = "alloc",
    not(any(
        all(target_arch = "aarch64", target_endian = "little"),
        all(target_arch = "arm", target_endian = "little"),
        target_arch = "x86_64"
    ))
))]
pub(super) fn limbs_from_mont_in_place(r: &mut [Limb], tmp: &mut [Limb], m: &[Limb], n0: &N0) {
    prefixed_extern! {
        fn bn_from_montgomery_in_place(
            r: *mut Limb,
            num_r: c::size_t,
            a: *mut Limb,
            num_a: c::size_t,
            n: *const Limb,
            num_n: c::size_t,
            n0: &N0,
        ) -> bssl::Result;
    }
    Result::from(unsafe {
        bn_from_montgomery_in_place(
            r.as_mut_ptr(),
            r.len(),
            tmp.as_mut_ptr(),
            tmp.len(),
            m.as_ptr(),
            m.len(),
            n0,
        )
    })
    .unwrap()
}

#[cfg(not(any(
    all(target_arch = "aarch64", target_endian = "little"),
    all(target_arch = "arm", target_endian = "little"),
    target_arch = "x86_64"
)))]
fn limbs_mul(r: &mut [Limb], a: &[Limb], b: &[Limb]) {
    debug_assert_eq!(r.len(), 2 * a.len());
    debug_assert_eq!(a.len(), b.len());
    let ab_len = a.len();

    r[..ab_len].fill(0);
    for (i, &b_limb) in b.iter().enumerate() {
        r[ab_len + i] = unsafe {
            limbs_mul_add_limb(r[i..][..ab_len].as_mut_ptr(), a.as_ptr(), b_limb, ab_len)
        };
    }
}

#[cfg(any(
    test,
    not(any(
        all(target_arch = "aarch64", target_endian = "little"),
        all(target_arch = "arm", target_endian = "little"),
        target_arch = "x86_64",
    ))
))]
prefixed_extern! {
    // `r` must not alias `a`
    #[must_use]
    fn limbs_mul_add_limb(r: *mut Limb, a: *const Limb, b: Limb, num_limbs: c::size_t) -> Limb;
}

/// r = r**2
pub(super) fn limbs_square_mont(
    r: &mut [Limb],
    n: &[Limb],
    n0: &N0,
    cpu: cpu::Features,
) -> Result<(), LimbSliceError> {
    #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
    {
        use super::limbs::aarch64;
        use crate::polyfill::slice;
        if let ((r, []), (n, [])) = (slice::as_chunks_mut(r), slice::as_chunks(n)) {
            return aarch64::mont::sqr_mont5(r, n, n0);
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        use super::limbs::x86_64;
        use crate::{cpu::GetFeature as _, polyfill::slice};
        if let ((r, []), (n, [])) = (slice::as_chunks_mut(r), slice::as_chunks(n)) {
            return x86_64::mont::sqr_mont5(r, n, n0, cpu.get_feature());
        }
    }

    limbs_mul_mont(r, n, n0, cpu)
}

#[cfg(test)]
mod tests {
    use super::super::MAX_LIMBS;
    use super::*;
    use crate::limb::Limb;

    #[test]
    // TODO: wasm
    fn test_mul_add_words() {
        const ZERO: Limb = 0;
        const MAX: Limb = ZERO.wrapping_sub(1);
        static TEST_CASES: &[(&[Limb], &[Limb], Limb, Limb, &[Limb])] = &[
            (&[0], &[0], 0, 0, &[0]),
            (&[MAX], &[0], MAX, 0, &[MAX]),
            (&[0], &[MAX], MAX, MAX - 1, &[1]),
            (&[MAX], &[MAX], MAX, MAX, &[0]),
            (&[0, 0], &[MAX, MAX], MAX, MAX - 1, &[1, MAX]),
            (&[1, 0], &[MAX, MAX], MAX, MAX - 1, &[2, MAX]),
            (&[MAX, 0], &[MAX, MAX], MAX, MAX, &[0, 0]),
            (&[0, 1], &[MAX, MAX], MAX, MAX, &[1, 0]),
            (&[MAX, MAX], &[MAX, MAX], MAX, MAX, &[0, MAX]),
        ];

        for (i, (r_input, a, w, expected_retval, expected_r)) in TEST_CASES.iter().enumerate() {
            let mut r = [0; MAX_LIMBS];
            let r = {
                let r = &mut r[..r_input.len()];
                r.copy_from_slice(r_input);
                r
            };
            assert_eq!(r.len(), a.len()); // Sanity check
            let actual_retval =
                unsafe { limbs_mul_add_limb(r.as_mut_ptr(), a.as_ptr(), *w, a.len()) };
            assert_eq!(&r, expected_r, "{}: {:x?} != {:x?}", i, r, expected_r);
            assert_eq!(
                actual_retval, *expected_retval,
                "{}: {:x?} != {:x?}",
                i, actual_retval, *expected_retval
            );
        }
    }
}
