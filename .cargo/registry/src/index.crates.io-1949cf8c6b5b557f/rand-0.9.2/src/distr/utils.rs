// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Math helper functions

#[cfg(feature = "simd_support")]
use core::simd::prelude::*;
#[cfg(feature = "simd_support")]
use core::simd::{LaneCount, SimdElement, SupportedLaneCount};

pub(crate) trait WideningMultiply<RHS = Self> {
    type Output;

    fn wmul(self, x: RHS) -> Self::Output;
}

macro_rules! wmul_impl {
    ($ty:ty, $wide:ty, $shift:expr) => {
        impl WideningMultiply for $ty {
            type Output = ($ty, $ty);

            #[inline(always)]
            fn wmul(self, x: $ty) -> Self::Output {
                let tmp = (self as $wide) * (x as $wide);
                ((tmp >> $shift) as $ty, tmp as $ty)
            }
        }
    };

    // simd bulk implementation
    ($(($ty:ident, $wide:ty),)+, $shift:expr) => {
        $(
            impl WideningMultiply for $ty {
                type Output = ($ty, $ty);

                #[inline(always)]
                fn wmul(self, x: $ty) -> Self::Output {
                    // For supported vectors, this should compile to a couple
                    // supported multiply & swizzle instructions (no actual
                    // casting).
                    // TODO: optimize
                    let y: $wide = self.cast();
                    let x: $wide = x.cast();
                    let tmp = y * x;
                    let hi: $ty = (tmp >> Simd::splat($shift)).cast();
                    let lo: $ty = tmp.cast();
                    (hi, lo)
                }
            }
        )+
    };
}
wmul_impl! { u8, u16, 8 }
wmul_impl! { u16, u32, 16 }
wmul_impl! { u32, u64, 32 }
wmul_impl! { u64, u128, 64 }

// This code is a translation of the __mulddi3 function in LLVM's
// compiler-rt. It is an optimised variant of the common method
// `(a + b) * (c + d) = ac + ad + bc + bd`.
//
// For some reason LLVM can optimise the C version very well, but
// keeps shuffling registers in this Rust translation.
macro_rules! wmul_impl_large {
    ($ty:ty, $half:expr) => {
        impl WideningMultiply for $ty {
            type Output = ($ty, $ty);

            #[inline(always)]
            fn wmul(self, b: $ty) -> Self::Output {
                const LOWER_MASK: $ty = !0 >> $half;
                let mut low = (self & LOWER_MASK).wrapping_mul(b & LOWER_MASK);
                let mut t = low >> $half;
                low &= LOWER_MASK;
                t += (self >> $half).wrapping_mul(b & LOWER_MASK);
                low += (t & LOWER_MASK) << $half;
                let mut high = t >> $half;
                t = low >> $half;
                low &= LOWER_MASK;
                t += (b >> $half).wrapping_mul(self & LOWER_MASK);
                low += (t & LOWER_MASK) << $half;
                high += t >> $half;
                high += (self >> $half).wrapping_mul(b >> $half);

                (high, low)
            }
        }
    };

    // simd bulk implementation
    (($($ty:ty,)+) $scalar:ty, $half:expr) => {
        $(
            impl WideningMultiply for $ty {
                type Output = ($ty, $ty);

                #[inline(always)]
                fn wmul(self, b: $ty) -> Self::Output {
                    // needs wrapping multiplication
                    let lower_mask = <$ty>::splat(!0 >> $half);
                    let half = <$ty>::splat($half);
                    let mut low = (self & lower_mask) * (b & lower_mask);
                    let mut t = low >> half;
                    low &= lower_mask;
                    t += (self >> half) * (b & lower_mask);
                    low += (t & lower_mask) << half;
                    let mut high = t >> half;
                    t = low >> half;
                    low &= lower_mask;
                    t += (b >> half) * (self & lower_mask);
                    low += (t & lower_mask) << half;
                    high += t >> half;
                    high += (self >> half) * (b >> half);

                    (high, low)
                }
            }
        )+
    };
}
wmul_impl_large! { u128, 64 }

macro_rules! wmul_impl_usize {
    ($ty:ty) => {
        impl WideningMultiply for usize {
            type Output = (usize, usize);

            #[inline(always)]
            fn wmul(self, x: usize) -> Self::Output {
                let (high, low) = (self as $ty).wmul(x as $ty);
                (high as usize, low as usize)
            }
        }
    };
}
#[cfg(target_pointer_width = "16")]
wmul_impl_usize! { u16 }
#[cfg(target_pointer_width = "32")]
wmul_impl_usize! { u32 }
#[cfg(target_pointer_width = "64")]
wmul_impl_usize! { u64 }

#[cfg(feature = "simd_support")]
mod simd_wmul {
    use super::*;
    #[cfg(target_arch = "x86")]
    use core::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::*;

    wmul_impl! {
        (u8x4, u16x4),
        (u8x8, u16x8),
        (u8x16, u16x16),
        (u8x32, u16x32),
        (u8x64, Simd<u16, 64>),,
        8
    }

    wmul_impl! { (u16x2, u32x2),, 16 }
    wmul_impl! { (u16x4, u32x4),, 16 }
    #[cfg(not(target_feature = "sse2"))]
    wmul_impl! { (u16x8, u32x8),, 16 }
    #[cfg(not(target_feature = "avx2"))]
    wmul_impl! { (u16x16, u32x16),, 16 }
    #[cfg(not(target_feature = "avx512bw"))]
    wmul_impl! { (u16x32, Simd<u32, 32>),, 16 }

    // 16-bit lane widths allow use of the x86 `mulhi` instructions, which
    // means `wmul` can be implemented with only two instructions.
    #[allow(unused_macros)]
    macro_rules! wmul_impl_16 {
        ($ty:ident, $mulhi:ident, $mullo:ident) => {
            impl WideningMultiply for $ty {
                type Output = ($ty, $ty);

                #[inline(always)]
                fn wmul(self, x: $ty) -> Self::Output {
                    let hi = unsafe { $mulhi(self.into(), x.into()) }.into();
                    let lo = unsafe { $mullo(self.into(), x.into()) }.into();
                    (hi, lo)
                }
            }
        };
    }

    #[cfg(target_feature = "sse2")]
    wmul_impl_16! { u16x8, _mm_mulhi_epu16, _mm_mullo_epi16 }
    #[cfg(target_feature = "avx2")]
    wmul_impl_16! { u16x16, _mm256_mulhi_epu16, _mm256_mullo_epi16 }
    #[cfg(target_feature = "avx512bw")]
    wmul_impl_16! { u16x32, _mm512_mulhi_epu16, _mm512_mullo_epi16 }

    wmul_impl! {
        (u32x2, u64x2),
        (u32x4, u64x4),
        (u32x8, u64x8),
        (u32x16, Simd<u64, 16>),,
        32
    }

    wmul_impl_large! { (u64x2, u64x4, u64x8,) u64, 32 }
}

/// Helper trait when dealing with scalar and SIMD floating point types.
pub(crate) trait FloatSIMDUtils {
    // `PartialOrd` for vectors compares lexicographically. We want to compare all
    // the individual SIMD lanes instead, and get the combined result over all
    // lanes. This is possible using something like `a.lt(b).all()`, but we
    // implement it as a trait so we can write the same code for `f32` and `f64`.
    // Only the comparison functions we need are implemented.
    fn all_lt(self, other: Self) -> bool;
    fn all_le(self, other: Self) -> bool;
    fn all_finite(self) -> bool;

    type Mask;
    fn gt_mask(self, other: Self) -> Self::Mask;

    // Decrease all lanes where the mask is `true` to the next lower value
    // representable by the floating-point type. At least one of the lanes
    // must be set.
    fn decrease_masked(self, mask: Self::Mask) -> Self;

    // Convert from int value. Conversion is done while retaining the numerical
    // value, not by retaining the binary representation.
    type UInt;
    fn cast_from_int(i: Self::UInt) -> Self;
}

#[cfg(test)]
pub(crate) trait FloatSIMDScalarUtils: FloatSIMDUtils {
    type Scalar;

    fn replace(self, index: usize, new_value: Self::Scalar) -> Self;
    fn extract_lane(self, index: usize) -> Self::Scalar;
}

/// Implement functions on f32/f64 to give them APIs similar to SIMD types
pub(crate) trait FloatAsSIMD: Sized {
    #[cfg(test)]
    const LEN: usize = 1;

    #[inline(always)]
    fn splat(scalar: Self) -> Self {
        scalar
    }
}

pub(crate) trait IntAsSIMD: Sized {
    #[inline(always)]
    fn splat(scalar: Self) -> Self {
        scalar
    }
}

impl IntAsSIMD for u32 {}
impl IntAsSIMD for u64 {}

pub(crate) trait BoolAsSIMD: Sized {
    fn any(self) -> bool;
}

impl BoolAsSIMD for bool {
    #[inline(always)]
    fn any(self) -> bool {
        self
    }
}

macro_rules! scalar_float_impl {
    ($ty:ident, $uty:ident) => {
        impl FloatSIMDUtils for $ty {
            type Mask = bool;
            type UInt = $uty;

            #[inline(always)]
            fn all_lt(self, other: Self) -> bool {
                self < other
            }

            #[inline(always)]
            fn all_le(self, other: Self) -> bool {
                self <= other
            }

            #[inline(always)]
            fn all_finite(self) -> bool {
                self.is_finite()
            }

            #[inline(always)]
            fn gt_mask(self, other: Self) -> Self::Mask {
                self > other
            }

            #[inline(always)]
            fn decrease_masked(self, mask: Self::Mask) -> Self {
                debug_assert!(mask, "At least one lane must be set");
                <$ty>::from_bits(self.to_bits() - 1)
            }

            #[inline]
            fn cast_from_int(i: Self::UInt) -> Self {
                i as $ty
            }
        }

        #[cfg(test)]
        impl FloatSIMDScalarUtils for $ty {
            type Scalar = $ty;

            #[inline]
            fn replace(self, index: usize, new_value: Self::Scalar) -> Self {
                debug_assert_eq!(index, 0);
                new_value
            }

            #[inline]
            fn extract_lane(self, index: usize) -> Self::Scalar {
                debug_assert_eq!(index, 0);
                self
            }
        }

        impl FloatAsSIMD for $ty {}
    };
}

scalar_float_impl!(f32, u32);
scalar_float_impl!(f64, u64);

#[cfg(feature = "simd_support")]
macro_rules! simd_impl {
    ($fty:ident, $uty:ident) => {
        impl<const LANES: usize> FloatSIMDUtils for Simd<$fty, LANES>
        where
            LaneCount<LANES>: SupportedLaneCount,
        {
            type Mask = Mask<<$fty as SimdElement>::Mask, LANES>;
            type UInt = Simd<$uty, LANES>;

            #[inline(always)]
            fn all_lt(self, other: Self) -> bool {
                self.simd_lt(other).all()
            }

            #[inline(always)]
            fn all_le(self, other: Self) -> bool {
                self.simd_le(other).all()
            }

            #[inline(always)]
            fn all_finite(self) -> bool {
                self.is_finite().all()
            }

            #[inline(always)]
            fn gt_mask(self, other: Self) -> Self::Mask {
                self.simd_gt(other)
            }

            #[inline(always)]
            fn decrease_masked(self, mask: Self::Mask) -> Self {
                // Casting a mask into ints will produce all bits set for
                // true, and 0 for false. Adding that to the binary
                // representation of a float means subtracting one from
                // the binary representation, resulting in the next lower
                // value representable by $fty. This works even when the
                // current value is infinity.
                debug_assert!(mask.any(), "At least one lane must be set");
                Self::from_bits(self.to_bits() + mask.to_int().cast())
            }

            #[inline]
            fn cast_from_int(i: Self::UInt) -> Self {
                i.cast()
            }
        }

        #[cfg(test)]
        impl<const LANES: usize> FloatSIMDScalarUtils for Simd<$fty, LANES>
        where
            LaneCount<LANES>: SupportedLaneCount,
        {
            type Scalar = $fty;

            #[inline]
            fn replace(mut self, index: usize, new_value: Self::Scalar) -> Self {
                self.as_mut_array()[index] = new_value;
                self
            }

            #[inline]
            fn extract_lane(self, index: usize) -> Self::Scalar {
                self.as_array()[index]
            }
        }
    };
}

#[cfg(feature = "simd_support")]
simd_impl!(f32, u32);
#[cfg(feature = "simd_support")]
simd_impl!(f64, u64);
