// Copyright 2018-2020 Developers of the Rand project.
// Copyright 2017 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! `UniformInt` implementation

use super::{Error, SampleBorrow, SampleUniform, UniformSampler};
use crate::distr::utils::WideningMultiply;
#[cfg(feature = "simd_support")]
use crate::distr::{Distribution, StandardUniform};
use crate::Rng;

#[cfg(feature = "simd_support")]
use core::simd::prelude::*;
#[cfg(feature = "simd_support")]
use core::simd::{LaneCount, SupportedLaneCount};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// The back-end implementing [`UniformSampler`] for integer types.
///
/// Unless you are implementing [`UniformSampler`] for your own type, this type
/// should not be used directly, use [`Uniform`] instead.
///
/// # Implementation notes
///
/// For simplicity, we use the same generic struct `UniformInt<X>` for all
/// integer types `X`. This gives us only one field type, `X`; to store unsigned
/// values of this size, we take use the fact that these conversions are no-ops.
///
/// For a closed range, the number of possible numbers we should generate is
/// `range = (high - low + 1)`. To avoid bias, we must ensure that the size of
/// our sample space, `zone`, is a multiple of `range`; other values must be
/// rejected (by replacing with a new random sample).
///
/// As a special case, we use `range = 0` to represent the full range of the
/// result type (i.e. for `new_inclusive($ty::MIN, $ty::MAX)`).
///
/// The optimum `zone` is the largest product of `range` which fits in our
/// (unsigned) target type. We calculate this by calculating how many numbers we
/// must reject: `reject = (MAX + 1) % range = (MAX - range + 1) % range`. Any (large)
/// product of `range` will suffice, thus in `sample_single` we multiply by a
/// power of 2 via bit-shifting (faster but may cause more rejections).
///
/// The smallest integer PRNGs generate is `u32`. For 8- and 16-bit outputs we
/// use `u32` for our `zone` and samples (because it's not slower and because
/// it reduces the chance of having to reject a sample). In this case we cannot
/// store `zone` in the target type since it is too large, however we know
/// `ints_to_reject < range <= $uty::MAX`.
///
/// An alternative to using a modulus is widening multiply: After a widening
/// multiply by `range`, the result is in the high word. Then comparing the low
/// word against `zone` makes sure our distribution is uniform.
///
/// # Bias
///
/// Unless the `unbiased` feature flag is used, outputs may have a small bias.
/// In the worst case, bias affects 1 in `2^n` samples where n is
/// 56 (`i8` and `u8`), 48 (`i16` and `u16`), 96 (`i32` and `u32`), 64 (`i64`
/// and `u64`), 128 (`i128` and `u128`).
///
/// [`Uniform`]: super::Uniform
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UniformInt<X> {
    pub(super) low: X,
    pub(super) range: X,
    thresh: X, // effectively 2.pow(max(64, uty_bits)) % range
}

macro_rules! uniform_int_impl {
    ($ty:ty, $uty:ty, $sample_ty:ident) => {
        impl SampleUniform for $ty {
            type Sampler = UniformInt<$ty>;
        }

        impl UniformSampler for UniformInt<$ty> {
            // We play free and fast with unsigned vs signed here
            // (when $ty is signed), but that's fine, since the
            // contract of this macro is for $ty and $uty to be
            // "bit-equal", so casting between them is a no-op.

            type X = $ty;

            #[inline] // if the range is constant, this helps LLVM to do the
                      // calculations at compile-time.
            fn new<B1, B2>(low_b: B1, high_b: B2) -> Result<Self, Error>
            where
                B1: SampleBorrow<Self::X> + Sized,
                B2: SampleBorrow<Self::X> + Sized,
            {
                let low = *low_b.borrow();
                let high = *high_b.borrow();
                if !(low < high) {
                    return Err(Error::EmptyRange);
                }
                UniformSampler::new_inclusive(low, high - 1)
            }

            #[inline] // if the range is constant, this helps LLVM to do the
                      // calculations at compile-time.
            fn new_inclusive<B1, B2>(low_b: B1, high_b: B2) -> Result<Self, Error>
            where
                B1: SampleBorrow<Self::X> + Sized,
                B2: SampleBorrow<Self::X> + Sized,
            {
                let low = *low_b.borrow();
                let high = *high_b.borrow();
                if !(low <= high) {
                    return Err(Error::EmptyRange);
                }

                let range = high.wrapping_sub(low).wrapping_add(1) as $uty;
                let thresh = if range > 0 {
                    let range = $sample_ty::from(range);
                    (range.wrapping_neg() % range)
                } else {
                    0
                };

                Ok(UniformInt {
                    low,
                    range: range as $ty,           // type: $uty
                    thresh: thresh as $uty as $ty, // type: $sample_ty
                })
            }

            /// Sample from distribution, Lemire's method, unbiased
            #[inline]
            fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
                let range = self.range as $uty as $sample_ty;
                if range == 0 {
                    return rng.random();
                }

                let thresh = self.thresh as $uty as $sample_ty;
                let hi = loop {
                    let (hi, lo) = rng.random::<$sample_ty>().wmul(range);
                    if lo >= thresh {
                        break hi;
                    }
                };
                self.low.wrapping_add(hi as $ty)
            }

            #[inline]
            fn sample_single<R: Rng + ?Sized, B1, B2>(
                low_b: B1,
                high_b: B2,
                rng: &mut R,
            ) -> Result<Self::X, Error>
            where
                B1: SampleBorrow<Self::X> + Sized,
                B2: SampleBorrow<Self::X> + Sized,
            {
                let low = *low_b.borrow();
                let high = *high_b.borrow();
                if !(low < high) {
                    return Err(Error::EmptyRange);
                }
                Self::sample_single_inclusive(low, high - 1, rng)
            }

            /// Sample single value, Canon's method, biased
            ///
            /// In the worst case, bias affects 1 in `2^n` samples where n is
            /// 56 (`i8`), 48 (`i16`), 96 (`i32`), 64 (`i64`), 128 (`i128`).
            #[cfg(not(feature = "unbiased"))]
            #[inline]
            fn sample_single_inclusive<R: Rng + ?Sized, B1, B2>(
                low_b: B1,
                high_b: B2,
                rng: &mut R,
            ) -> Result<Self::X, Error>
            where
                B1: SampleBorrow<Self::X> + Sized,
                B2: SampleBorrow<Self::X> + Sized,
            {
                let low = *low_b.borrow();
                let high = *high_b.borrow();
                if !(low <= high) {
                    return Err(Error::EmptyRange);
                }
                let range = high.wrapping_sub(low).wrapping_add(1) as $uty as $sample_ty;
                if range == 0 {
                    // Range is MAX+1 (unrepresentable), so we need a special case
                    return Ok(rng.random());
                }

                // generate a sample using a sensible integer type
                let (mut result, lo_order) = rng.random::<$sample_ty>().wmul(range);

                // if the sample is biased...
                if lo_order > range.wrapping_neg() {
                    // ...generate a new sample to reduce bias...
                    let (new_hi_order, _) = (rng.random::<$sample_ty>()).wmul(range as $sample_ty);
                    // ... incrementing result on overflow
                    let is_overflow = lo_order.checked_add(new_hi_order as $sample_ty).is_none();
                    result += is_overflow as $sample_ty;
                }

                Ok(low.wrapping_add(result as $ty))
            }

            /// Sample single value, Canon's method, unbiased
            #[cfg(feature = "unbiased")]
            #[inline]
            fn sample_single_inclusive<R: Rng + ?Sized, B1, B2>(
                low_b: B1,
                high_b: B2,
                rng: &mut R,
            ) -> Result<Self::X, Error>
            where
                B1: SampleBorrow<$ty> + Sized,
                B2: SampleBorrow<$ty> + Sized,
            {
                let low = *low_b.borrow();
                let high = *high_b.borrow();
                if !(low <= high) {
                    return Err(Error::EmptyRange);
                }
                let range = high.wrapping_sub(low).wrapping_add(1) as $uty as $sample_ty;
                if range == 0 {
                    // Range is MAX+1 (unrepresentable), so we need a special case
                    return Ok(rng.random());
                }

                let (mut result, mut lo) = rng.random::<$sample_ty>().wmul(range);

                // In contrast to the biased sampler, we use a loop:
                while lo > range.wrapping_neg() {
                    let (new_hi, new_lo) = (rng.random::<$sample_ty>()).wmul(range);
                    match lo.checked_add(new_hi) {
                        Some(x) if x < $sample_ty::MAX => {
                            // Anything less than MAX: last term is 0
                            break;
                        }
                        None => {
                            // Overflow: last term is 1
                            result += 1;
                            break;
                        }
                        _ => {
                            // Unlikely case: must check next sample
                            lo = new_lo;
                            continue;
                        }
                    }
                }

                Ok(low.wrapping_add(result as $ty))
            }
        }
    };
}

uniform_int_impl! { i8, u8, u32 }
uniform_int_impl! { i16, u16, u32 }
uniform_int_impl! { i32, u32, u32 }
uniform_int_impl! { i64, u64, u64 }
uniform_int_impl! { i128, u128, u128 }
uniform_int_impl! { u8, u8, u32 }
uniform_int_impl! { u16, u16, u32 }
uniform_int_impl! { u32, u32, u32 }
uniform_int_impl! { u64, u64, u64 }
uniform_int_impl! { u128, u128, u128 }

#[cfg(feature = "simd_support")]
macro_rules! uniform_simd_int_impl {
    ($ty:ident, $unsigned:ident) => {
        // The "pick the largest zone that can fit in an `u32`" optimization
        // is less useful here. Multiple lanes complicate things, we don't
        // know the PRNG's minimal output size, and casting to a larger vector
        // is generally a bad idea for SIMD performance. The user can still
        // implement it manually.

        #[cfg(feature = "simd_support")]
        impl<const LANES: usize> SampleUniform for Simd<$ty, LANES>
        where
            LaneCount<LANES>: SupportedLaneCount,
            Simd<$unsigned, LANES>:
                WideningMultiply<Output = (Simd<$unsigned, LANES>, Simd<$unsigned, LANES>)>,
            StandardUniform: Distribution<Simd<$unsigned, LANES>>,
        {
            type Sampler = UniformInt<Simd<$ty, LANES>>;
        }

        #[cfg(feature = "simd_support")]
        impl<const LANES: usize> UniformSampler for UniformInt<Simd<$ty, LANES>>
        where
            LaneCount<LANES>: SupportedLaneCount,
            Simd<$unsigned, LANES>:
                WideningMultiply<Output = (Simd<$unsigned, LANES>, Simd<$unsigned, LANES>)>,
            StandardUniform: Distribution<Simd<$unsigned, LANES>>,
        {
            type X = Simd<$ty, LANES>;

            #[inline] // if the range is constant, this helps LLVM to do the
                      // calculations at compile-time.
            fn new<B1, B2>(low_b: B1, high_b: B2) -> Result<Self, Error>
                where B1: SampleBorrow<Self::X> + Sized,
                      B2: SampleBorrow<Self::X> + Sized
            {
                let low = *low_b.borrow();
                let high = *high_b.borrow();
                if !(low.simd_lt(high).all()) {
                    return Err(Error::EmptyRange);
                }
                UniformSampler::new_inclusive(low, high - Simd::splat(1))
            }

            #[inline] // if the range is constant, this helps LLVM to do the
                      // calculations at compile-time.
            fn new_inclusive<B1, B2>(low_b: B1, high_b: B2) -> Result<Self, Error>
                where B1: SampleBorrow<Self::X> + Sized,
                      B2: SampleBorrow<Self::X> + Sized
            {
                let low = *low_b.borrow();
                let high = *high_b.borrow();
                if !(low.simd_le(high).all()) {
                    return Err(Error::EmptyRange);
                }

                // NOTE: all `Simd` operations are inherently wrapping,
                //       see https://doc.rust-lang.org/std/simd/struct.Simd.html
                let range: Simd<$unsigned, LANES> = ((high - low) + Simd::splat(1)).cast();

                // We must avoid divide-by-zero by using 0 % 1 == 0.
                let not_full_range = range.simd_gt(Simd::splat(0));
                let modulo = not_full_range.select(range, Simd::splat(1));
                let ints_to_reject = range.wrapping_neg() % modulo;

                Ok(UniformInt {
                    low,
                    // These are really $unsigned values, but store as $ty:
                    range: range.cast(),
                    thresh: ints_to_reject.cast(),
                })
            }

            fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
                let range: Simd<$unsigned, LANES> = self.range.cast();
                let thresh: Simd<$unsigned, LANES> = self.thresh.cast();

                // This might seem very slow, generating a whole new
                // SIMD vector for every sample rejection. For most uses
                // though, the chance of rejection is small and provides good
                // general performance. With multiple lanes, that chance is
                // multiplied. To mitigate this, we replace only the lanes of
                // the vector which fail, iteratively reducing the chance of
                // rejection. The replacement method does however add a little
                // overhead. Benchmarking or calculating probabilities might
                // reveal contexts where this replacement method is slower.
                let mut v: Simd<$unsigned, LANES> = rng.random();
                loop {
                    let (hi, lo) = v.wmul(range);
                    let mask = lo.simd_ge(thresh);
                    if mask.all() {
                        let hi: Simd<$ty, LANES> = hi.cast();
                        // wrapping addition
                        let result = self.low + hi;
                        // `select` here compiles to a blend operation
                        // When `range.eq(0).none()` the compare and blend
                        // operations are avoided.
                        let v: Simd<$ty, LANES> = v.cast();
                        return range.simd_gt(Simd::splat(0)).select(result, v);
                    }
                    // Replace only the failing lanes
                    v = mask.select(v, rng.random());
                }
            }
        }
    };

    // bulk implementation
    ($(($unsigned:ident, $signed:ident)),+) => {
        $(
            uniform_simd_int_impl!($unsigned, $unsigned);
            uniform_simd_int_impl!($signed, $unsigned);
        )+
    };
}

#[cfg(feature = "simd_support")]
uniform_simd_int_impl! { (u8, i8), (u16, i16), (u32, i32), (u64, i64) }

/// The back-end implementing [`UniformSampler`] for `usize`.
///
/// # Implementation notes
///
/// Sampling a `usize` value is usually used in relation to the length of an
/// array or other memory structure, thus it is reasonable to assume that the
/// vast majority of use-cases will have a maximum size under [`u32::MAX`].
/// In part to optimise for this use-case, but mostly to ensure that results
/// are portable across 32-bit and 64-bit architectures (as far as is possible),
/// this implementation will use 32-bit sampling when possible.
#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(all(feature = "serde"), derive(Serialize))]
// To be able to deserialize on 32-bit we need to replace this with a custom
// implementation of the Deserialize trait, to be able to:
// - panic when `mode64` is `true` on 32-bit,
// - assign the default value to `mode64` when it's missing on 64-bit,
// - panic when the `usize` fields are greater than `u32::MAX` on 32-bit.
#[cfg_attr(
    all(feature = "serde", target_pointer_width = "64"),
    derive(Deserialize)
)]
pub struct UniformUsize {
    /// The lowest possible value.
    low: usize,
    /// The number of possible values. `0` has a special meaning: all.
    range: usize,
    /// Threshold used when sampling to obtain a uniform distribution.
    thresh: usize,
    /// Whether the largest possible value is greater than `u32::MAX`.
    #[cfg(target_pointer_width = "64")]
    // Handle missing field when deserializing on 64-bit an object serialized
    // on 32-bit. Can be removed when switching to a custom deserializer.
    #[cfg_attr(feature = "serde", serde(default))]
    mode64: bool,
}

#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
impl SampleUniform for usize {
    type Sampler = UniformUsize;
}

#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
impl UniformSampler for UniformUsize {
    type X = usize;

    #[inline] // if the range is constant, this helps LLVM to do the
              // calculations at compile-time.
    fn new<B1, B2>(low_b: B1, high_b: B2) -> Result<Self, Error>
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        let low = *low_b.borrow();
        let high = *high_b.borrow();
        if !(low < high) {
            return Err(Error::EmptyRange);
        }

        UniformSampler::new_inclusive(low, high - 1)
    }

    #[inline] // if the range is constant, this helps LLVM to do the
              // calculations at compile-time.
    fn new_inclusive<B1, B2>(low_b: B1, high_b: B2) -> Result<Self, Error>
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        let low = *low_b.borrow();
        let high = *high_b.borrow();
        if !(low <= high) {
            return Err(Error::EmptyRange);
        }

        #[cfg(target_pointer_width = "64")]
        let mode64 = high > (u32::MAX as usize);
        #[cfg(target_pointer_width = "32")]
        let mode64 = false;

        let (range, thresh);
        if cfg!(target_pointer_width = "64") && !mode64 {
            let range32 = (high as u32).wrapping_sub(low as u32).wrapping_add(1);
            range = range32 as usize;
            thresh = if range32 > 0 {
                (range32.wrapping_neg() % range32) as usize
            } else {
                0
            };
        } else {
            range = high.wrapping_sub(low).wrapping_add(1);
            thresh = if range > 0 {
                range.wrapping_neg() % range
            } else {
                0
            };
        }

        Ok(UniformUsize {
            low,
            range,
            thresh,
            #[cfg(target_pointer_width = "64")]
            mode64,
        })
    }

    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> usize {
        #[cfg(target_pointer_width = "32")]
        let mode32 = true;
        #[cfg(target_pointer_width = "64")]
        let mode32 = !self.mode64;

        if mode32 {
            let range = self.range as u32;
            if range == 0 {
                return rng.random::<u32>() as usize;
            }

            let thresh = self.thresh as u32;
            let hi = loop {
                let (hi, lo) = rng.random::<u32>().wmul(range);
                if lo >= thresh {
                    break hi;
                }
            };
            self.low.wrapping_add(hi as usize)
        } else {
            let range = self.range as u64;
            if range == 0 {
                return rng.random::<u64>() as usize;
            }

            let thresh = self.thresh as u64;
            let hi = loop {
                let (hi, lo) = rng.random::<u64>().wmul(range);
                if lo >= thresh {
                    break hi;
                }
            };
            self.low.wrapping_add(hi as usize)
        }
    }

    #[inline]
    fn sample_single<R: Rng + ?Sized, B1, B2>(
        low_b: B1,
        high_b: B2,
        rng: &mut R,
    ) -> Result<Self::X, Error>
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        let low = *low_b.borrow();
        let high = *high_b.borrow();
        if !(low < high) {
            return Err(Error::EmptyRange);
        }

        if cfg!(target_pointer_width = "64") && high > (u32::MAX as usize) {
            return UniformInt::<u64>::sample_single(low as u64, high as u64, rng)
                .map(|x| x as usize);
        }

        UniformInt::<u32>::sample_single(low as u32, high as u32, rng).map(|x| x as usize)
    }

    #[inline]
    fn sample_single_inclusive<R: Rng + ?Sized, B1, B2>(
        low_b: B1,
        high_b: B2,
        rng: &mut R,
    ) -> Result<Self::X, Error>
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        let low = *low_b.borrow();
        let high = *high_b.borrow();
        if !(low <= high) {
            return Err(Error::EmptyRange);
        }

        if cfg!(target_pointer_width = "64") && high > (u32::MAX as usize) {
            return UniformInt::<u64>::sample_single_inclusive(low as u64, high as u64, rng)
                .map(|x| x as usize);
        }

        UniformInt::<u32>::sample_single_inclusive(low as u32, high as u32, rng).map(|x| x as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distr::{Distribution, Uniform};
    use core::fmt::Debug;
    use core::ops::Add;

    #[test]
    fn test_uniform_bad_limits_equal_int() {
        assert_eq!(Uniform::new(10, 10), Err(Error::EmptyRange));
    }

    #[test]
    fn test_uniform_good_limits_equal_int() {
        let mut rng = crate::test::rng(804);
        let dist = Uniform::new_inclusive(10, 10).unwrap();
        for _ in 0..20 {
            assert_eq!(rng.sample(dist), 10);
        }
    }

    #[test]
    fn test_uniform_bad_limits_flipped_int() {
        assert_eq!(Uniform::new(10, 5), Err(Error::EmptyRange));
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_integers() {
        let mut rng = crate::test::rng(251);
        macro_rules! t {
            ($ty:ident, $v:expr, $le:expr, $lt:expr) => {{
                for &(low, high) in $v.iter() {
                    let my_uniform = Uniform::new(low, high).unwrap();
                    for _ in 0..1000 {
                        let v: $ty = rng.sample(my_uniform);
                        assert!($le(low, v) && $lt(v, high));
                    }

                    let my_uniform = Uniform::new_inclusive(low, high).unwrap();
                    for _ in 0..1000 {
                        let v: $ty = rng.sample(my_uniform);
                        assert!($le(low, v) && $le(v, high));
                    }

                    let my_uniform = Uniform::new(&low, high).unwrap();
                    for _ in 0..1000 {
                        let v: $ty = rng.sample(my_uniform);
                        assert!($le(low, v) && $lt(v, high));
                    }

                    let my_uniform = Uniform::new_inclusive(&low, &high).unwrap();
                    for _ in 0..1000 {
                        let v: $ty = rng.sample(my_uniform);
                        assert!($le(low, v) && $le(v, high));
                    }

                    for _ in 0..1000 {
                        let v = <$ty as SampleUniform>::Sampler::sample_single(low, high, &mut rng).unwrap();
                        assert!($le(low, v) && $lt(v, high));
                    }

                    for _ in 0..1000 {
                        let v = <$ty as SampleUniform>::Sampler::sample_single_inclusive(low, high, &mut rng).unwrap();
                        assert!($le(low, v) && $le(v, high));
                    }
                }
            }};

            // scalar bulk
            ($($ty:ident),*) => {{
                $(t!(
                    $ty,
                    [(0, 10), (10, 127), ($ty::MIN, $ty::MAX)],
                    |x, y| x <= y,
                    |x, y| x < y
                );)*
            }};

            // simd bulk
            ($($ty:ident),* => $scalar:ident) => {{
                $(t!(
                    $ty,
                    [
                        ($ty::splat(0), $ty::splat(10)),
                        ($ty::splat(10), $ty::splat(127)),
                        ($ty::splat($scalar::MIN), $ty::splat($scalar::MAX)),
                    ],
                    |x: $ty, y| x.simd_le(y).all(),
                    |x: $ty, y| x.simd_lt(y).all()
                );)*
            }};
        }
        t!(i8, i16, i32, i64, i128, u8, u16, u32, u64, usize, u128);

        #[cfg(feature = "simd_support")]
        {
            t!(u8x4, u8x8, u8x16, u8x32, u8x64 => u8);
            t!(i8x4, i8x8, i8x16, i8x32, i8x64 => i8);
            t!(u16x2, u16x4, u16x8, u16x16, u16x32 => u16);
            t!(i16x2, i16x4, i16x8, i16x16, i16x32 => i16);
            t!(u32x2, u32x4, u32x8, u32x16 => u32);
            t!(i32x2, i32x4, i32x8, i32x16 => i32);
            t!(u64x2, u64x4, u64x8 => u64);
            t!(i64x2, i64x4, i64x8 => i64);
        }
    }

    #[test]
    fn test_uniform_from_std_range() {
        let r = Uniform::try_from(2u32..7).unwrap();
        assert_eq!(r.0.low, 2);
        assert_eq!(r.0.range, 5);
    }

    #[test]
    fn test_uniform_from_std_range_bad_limits() {
        #![allow(clippy::reversed_empty_ranges)]
        assert!(Uniform::try_from(100..10).is_err());
        assert!(Uniform::try_from(100..100).is_err());
    }

    #[test]
    fn test_uniform_from_std_range_inclusive() {
        let r = Uniform::try_from(2u32..=6).unwrap();
        assert_eq!(r.0.low, 2);
        assert_eq!(r.0.range, 5);
    }

    #[test]
    fn test_uniform_from_std_range_inclusive_bad_limits() {
        #![allow(clippy::reversed_empty_ranges)]
        assert!(Uniform::try_from(100..=10).is_err());
        assert!(Uniform::try_from(100..=99).is_err());
    }

    #[test]
    fn value_stability() {
        fn test_samples<T: SampleUniform + Copy + Debug + PartialEq + Add<T>>(
            lb: T,
            ub: T,
            ub_excl: T,
            expected: &[T],
        ) where
            Uniform<T>: Distribution<T>,
        {
            let mut rng = crate::test::rng(897);
            let mut buf = [lb; 6];

            for x in &mut buf[0..3] {
                *x = T::Sampler::sample_single_inclusive(lb, ub, &mut rng).unwrap();
            }

            let distr = Uniform::new_inclusive(lb, ub).unwrap();
            for x in &mut buf[3..6] {
                *x = rng.sample(&distr);
            }
            assert_eq!(&buf, expected);

            let mut rng = crate::test::rng(897);

            for x in &mut buf[0..3] {
                *x = T::Sampler::sample_single(lb, ub_excl, &mut rng).unwrap();
            }

            let distr = Uniform::new(lb, ub_excl).unwrap();
            for x in &mut buf[3..6] {
                *x = rng.sample(&distr);
            }
            assert_eq!(&buf, expected);
        }

        test_samples(-105i8, 111, 112, &[-99, -48, 107, 72, -19, 56]);
        test_samples(2i16, 1352, 1353, &[43, 361, 1325, 1109, 539, 1005]);
        test_samples(
            -313853i32,
            13513,
            13514,
            &[-303803, -226673, 6912, -45605, -183505, -70668],
        );
        test_samples(
            131521i64,
            6542165,
            6542166,
            &[1838724, 5384489, 4893692, 3712948, 3951509, 4094926],
        );
        test_samples(
            -0x8000_0000_0000_0000_0000_0000_0000_0000i128,
            -1,
            0,
            &[
                -30725222750250982319765550926688025855,
                -75088619368053423329503924805178012357,
                -64950748766625548510467638647674468829,
                -41794017901603587121582892414659436495,
                -63623852319608406524605295913876414006,
                -17404679390297612013597359206379189023,
            ],
        );
        test_samples(11u8, 218, 219, &[17, 66, 214, 181, 93, 165]);
        test_samples(11u16, 218, 219, &[17, 66, 214, 181, 93, 165]);
        test_samples(11u32, 218, 219, &[17, 66, 214, 181, 93, 165]);
        test_samples(11u64, 218, 219, &[66, 181, 165, 127, 134, 139]);
        test_samples(11u128, 218, 219, &[181, 127, 139, 167, 141, 197]);
        test_samples(11usize, 218, 219, &[17, 66, 214, 181, 93, 165]);

        #[cfg(feature = "simd_support")]
        {
            let lb = Simd::from([11u8, 0, 128, 127]);
            let ub = Simd::from([218, 254, 254, 254]);
            let ub_excl = ub + Simd::splat(1);
            test_samples(
                lb,
                ub,
                ub_excl,
                &[
                    Simd::from([13, 5, 237, 130]),
                    Simd::from([126, 186, 149, 161]),
                    Simd::from([103, 86, 234, 252]),
                    Simd::from([35, 18, 225, 231]),
                    Simd::from([106, 153, 246, 177]),
                    Simd::from([195, 168, 149, 222]),
                ],
            );
        }
    }

    #[test]
    fn test_uniform_usize_empty_range() {
        assert_eq!(UniformUsize::new(10, 10), Err(Error::EmptyRange));
        assert!(UniformUsize::new(10, 11).is_ok());

        assert_eq!(UniformUsize::new_inclusive(10, 9), Err(Error::EmptyRange));
        assert!(UniformUsize::new_inclusive(10, 10).is_ok());
    }

    #[test]
    fn test_uniform_usize_constructors() {
        assert_eq!(
            UniformUsize::new_inclusive(u32::MAX as usize, u32::MAX as usize),
            Ok(UniformUsize {
                low: u32::MAX as usize,
                range: 1,
                thresh: 0,
                #[cfg(target_pointer_width = "64")]
                mode64: false
            })
        );
        assert_eq!(
            UniformUsize::new_inclusive(0, u32::MAX as usize),
            Ok(UniformUsize {
                low: 0,
                range: 0,
                thresh: 0,
                #[cfg(target_pointer_width = "64")]
                mode64: false
            })
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            UniformUsize::new_inclusive(0, u32::MAX as usize + 1),
            Ok(UniformUsize {
                low: 0,
                range: u32::MAX as usize + 2,
                thresh: 1,
                mode64: true
            })
        );
        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            UniformUsize::new_inclusive(u32::MAX as usize, u64::MAX as usize),
            Ok(UniformUsize {
                low: u32::MAX as usize,
                range: u64::MAX as usize - u32::MAX as usize + 1,
                thresh: u32::MAX as usize,
                mode64: true
            })
        );
    }

    // This could be run also on 32-bit when deserialization is implemented.
    #[cfg(all(feature = "serde", target_pointer_width = "64"))]
    #[test]
    fn test_uniform_usize_deserialization() {
        use serde_json;
        let original = UniformUsize::new_inclusive(10, 100).expect("creation");
        let serialized = serde_json::to_string(&original).expect("serialization");
        let deserialized: UniformUsize =
            serde_json::from_str(&serialized).expect("deserialization");
        assert_eq!(deserialized, original);
    }

    #[cfg(all(feature = "serde", target_pointer_width = "64"))]
    #[test]
    fn test_uniform_usize_deserialization_from_32bit() {
        use serde_json;
        let serialized_on_32bit = r#"{"low":10,"range":91,"thresh":74}"#;
        let deserialized: UniformUsize =
            serde_json::from_str(&serialized_on_32bit).expect("deserialization");
        assert_eq!(
            deserialized,
            UniformUsize::new_inclusive(10, 100).expect("creation")
        );
    }

    #[cfg(all(feature = "serde", target_pointer_width = "64"))]
    #[test]
    fn test_uniform_usize_deserialization_64bit() {
        use serde_json;
        let original = UniformUsize::new_inclusive(1, u64::MAX as usize - 1).expect("creation");
        assert!(original.mode64);
        let serialized = serde_json::to_string(&original).expect("serialization");
        let deserialized: UniformUsize =
            serde_json::from_str(&serialized).expect("deserialization");
        assert_eq!(deserialized, original);
    }
}
