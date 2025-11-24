// Copyright 2018-2020 Developers of the Rand project.
// Copyright 2017 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A distribution uniformly sampling numbers within a given range.
//!
//! [`Uniform`] is the standard distribution to sample uniformly from a range;
//! e.g. `Uniform::new_inclusive(1, 6).unwrap()` can sample integers from 1 to 6, like a
//! standard die. [`Rng::random_range`] is implemented over [`Uniform`].
//!
//! # Example usage
//!
//! ```
//! use rand::Rng;
//! use rand::distr::Uniform;
//!
//! let mut rng = rand::rng();
//! let side = Uniform::new(-10.0, 10.0).unwrap();
//!
//! // sample between 1 and 10 points
//! for _ in 0..rng.random_range(1..=10) {
//!     // sample a point from the square with sides -10 - 10 in two dimensions
//!     let (x, y) = (rng.sample(side), rng.sample(side));
//!     println!("Point: {}, {}", x, y);
//! }
//! ```
//!
//! # Extending `Uniform` to support a custom type
//!
//! To extend [`Uniform`] to support your own types, write a back-end which
//! implements the [`UniformSampler`] trait, then implement the [`SampleUniform`]
//! helper trait to "register" your back-end. See the `MyF32` example below.
//!
//! At a minimum, the back-end needs to store any parameters needed for sampling
//! (e.g. the target range) and implement `new`, `new_inclusive` and `sample`.
//! Those methods should include an assertion to check the range is valid (i.e.
//! `low < high`). The example below merely wraps another back-end.
//!
//! The `new`, `new_inclusive`, `sample_single` and `sample_single_inclusive`
//! functions use arguments of
//! type `SampleBorrow<X>` to support passing in values by reference or
//! by value. In the implementation of these functions, you can choose to
//! simply use the reference returned by [`SampleBorrow::borrow`], or you can choose
//! to copy or clone the value, whatever is appropriate for your type.
//!
//! ```
//! use rand::prelude::*;
//! use rand::distr::uniform::{Uniform, SampleUniform,
//!         UniformSampler, UniformFloat, SampleBorrow, Error};
//!
//! struct MyF32(f32);
//!
//! #[derive(Clone, Copy, Debug)]
//! struct UniformMyF32(UniformFloat<f32>);
//!
//! impl UniformSampler for UniformMyF32 {
//!     type X = MyF32;
//!
//!     fn new<B1, B2>(low: B1, high: B2) -> Result<Self, Error>
//!         where B1: SampleBorrow<Self::X> + Sized,
//!               B2: SampleBorrow<Self::X> + Sized
//!     {
//!         UniformFloat::<f32>::new(low.borrow().0, high.borrow().0).map(UniformMyF32)
//!     }
//!     fn new_inclusive<B1, B2>(low: B1, high: B2) -> Result<Self, Error>
//!         where B1: SampleBorrow<Self::X> + Sized,
//!               B2: SampleBorrow<Self::X> + Sized
//!     {
//!         UniformFloat::<f32>::new_inclusive(low.borrow().0, high.borrow().0).map(UniformMyF32)
//!     }
//!     fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
//!         MyF32(self.0.sample(rng))
//!     }
//! }
//!
//! impl SampleUniform for MyF32 {
//!     type Sampler = UniformMyF32;
//! }
//!
//! let (low, high) = (MyF32(17.0f32), MyF32(22.0f32));
//! let uniform = Uniform::new(low, high).unwrap();
//! let x = uniform.sample(&mut rand::rng());
//! ```
//!
//! [`SampleUniform`]: crate::distr::uniform::SampleUniform
//! [`UniformSampler`]: crate::distr::uniform::UniformSampler
//! [`UniformInt`]: crate::distr::uniform::UniformInt
//! [`UniformFloat`]: crate::distr::uniform::UniformFloat
//! [`UniformDuration`]: crate::distr::uniform::UniformDuration
//! [`SampleBorrow::borrow`]: crate::distr::uniform::SampleBorrow::borrow

#[path = "uniform_float.rs"]
mod float;
#[doc(inline)]
pub use float::UniformFloat;

#[path = "uniform_int.rs"]
mod int;
#[doc(inline)]
pub use int::{UniformInt, UniformUsize};

#[path = "uniform_other.rs"]
mod other;
#[doc(inline)]
pub use other::{UniformChar, UniformDuration};

use core::fmt;
use core::ops::{Range, RangeInclusive, RangeTo, RangeToInclusive};

use crate::distr::Distribution;
use crate::{Rng, RngCore};

/// Error type returned from [`Uniform::new`] and `new_inclusive`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// `low > high`, or equal in case of exclusive range.
    EmptyRange,
    /// Input or range `high - low` is non-finite. Not relevant to integer types.
    NonFinite,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::EmptyRange => "low > high (or equal if exclusive) in uniform distribution",
            Error::NonFinite => "Non-finite range in uniform distribution",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Sample values uniformly between two bounds.
///
/// # Construction
///
/// [`Uniform::new`] and [`Uniform::new_inclusive`] construct a uniform
/// distribution sampling from the given `low` and `high` limits. `Uniform` may
/// also be constructed via [`TryFrom`] as in `Uniform::try_from(1..=6).unwrap()`.
///
/// Constructors may do extra work up front to allow faster sampling of multiple
/// values. Where only a single sample is required it is suggested to use
/// [`Rng::random_range`] or one of the `sample_single` methods instead.
///
/// When sampling from a constant range, many calculations can happen at
/// compile-time and all methods should be fast; for floating-point ranges and
/// the full range of integer types, this should have comparable performance to
/// the [`StandardUniform`](super::StandardUniform) distribution.
///
/// # Provided implementations
///
/// - `char` ([`UniformChar`]): samples a range over the implementation for `u32`
/// - `f32`, `f64` ([`UniformFloat`]): samples approximately uniformly within a
///   range; bias may be present in the least-significant bit of the significand
///   and the limits of the input range may be sampled even when an open
///   (exclusive) range is used
/// - Integer types ([`UniformInt`]) may show a small bias relative to the
///   expected uniform distribution of output. In the worst case, bias affects
///   1 in `2^n` samples where n is 56 (`i8` and `u8`), 48 (`i16` and `u16`), 96
///   (`i32` and `u32`), 64 (`i64` and `u64`), 128 (`i128` and `u128`).
///   The `unbiased` feature flag fixes this bias.
/// - `usize` ([`UniformUsize`]) is handled specially, using the `u32`
///   implementation where possible to enable portable results across 32-bit and
///   64-bit CPU architectures.
/// - `Duration` ([`UniformDuration`]): samples a range over the implementation
///   for `u32` or `u64`
/// - SIMD types (requires [`simd_support`] feature) like x86's [`__m128i`]
///   and `std::simd`'s [`u32x4`], [`f32x4`] and [`mask32x4`] types are
///   effectively arrays of integer or floating-point types. Each lane is
///   sampled independently from its own range, potentially with more efficient
///   random-bit-usage than would be achieved with sequential sampling.
///
/// # Example
///
/// ```
/// use rand::distr::{Distribution, Uniform};
///
/// let between = Uniform::try_from(10..10000).unwrap();
/// let mut rng = rand::rng();
/// let mut sum = 0;
/// for _ in 0..1000 {
///     sum += between.sample(&mut rng);
/// }
/// println!("{}", sum);
/// ```
///
/// For a single sample, [`Rng::random_range`] may be preferred:
///
/// ```
/// use rand::Rng;
///
/// let mut rng = rand::rng();
/// println!("{}", rng.random_range(0..10));
/// ```
///
/// [`new`]: Uniform::new
/// [`new_inclusive`]: Uniform::new_inclusive
/// [`Rng::random_range`]: Rng::random_range
/// [`__m128i`]: https://doc.rust-lang.org/core/arch/x86/struct.__m128i.html
/// [`u32x4`]: std::simd::u32x4
/// [`f32x4`]: std::simd::f32x4
/// [`mask32x4`]: std::simd::mask32x4
/// [`simd_support`]: https://github.com/rust-random/rand#crate-features
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(bound(serialize = "X::Sampler: Serialize")))]
#[cfg_attr(
    feature = "serde",
    serde(bound(deserialize = "X::Sampler: Deserialize<'de>"))
)]
pub struct Uniform<X: SampleUniform>(X::Sampler);

impl<X: SampleUniform> Uniform<X> {
    /// Create a new `Uniform` instance, which samples uniformly from the half
    /// open range `[low, high)` (excluding `high`).
    ///
    /// For discrete types (e.g. integers), samples will always be strictly less
    /// than `high`. For (approximations of) continuous types (e.g. `f32`, `f64`),
    /// samples may equal `high` due to loss of precision but may not be
    /// greater than `high`.
    ///
    /// Fails if `low >= high`, or if `low`, `high` or the range `high - low` is
    /// non-finite. In release mode, only the range is checked.
    pub fn new<B1, B2>(low: B1, high: B2) -> Result<Uniform<X>, Error>
    where
        B1: SampleBorrow<X> + Sized,
        B2: SampleBorrow<X> + Sized,
    {
        X::Sampler::new(low, high).map(Uniform)
    }

    /// Create a new `Uniform` instance, which samples uniformly from the closed
    /// range `[low, high]` (inclusive).
    ///
    /// Fails if `low > high`, or if `low`, `high` or the range `high - low` is
    /// non-finite. In release mode, only the range is checked.
    pub fn new_inclusive<B1, B2>(low: B1, high: B2) -> Result<Uniform<X>, Error>
    where
        B1: SampleBorrow<X> + Sized,
        B2: SampleBorrow<X> + Sized,
    {
        X::Sampler::new_inclusive(low, high).map(Uniform)
    }
}

impl<X: SampleUniform> Distribution<X> for Uniform<X> {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> X {
        self.0.sample(rng)
    }
}

/// Helper trait for creating objects using the correct implementation of
/// [`UniformSampler`] for the sampling type.
///
/// See the [module documentation] on how to implement [`Uniform`] range
/// sampling for a custom type.
///
/// [module documentation]: crate::distr::uniform
pub trait SampleUniform: Sized {
    /// The `UniformSampler` implementation supporting type `X`.
    type Sampler: UniformSampler<X = Self>;
}

/// Helper trait handling actual uniform sampling.
///
/// See the [module documentation] on how to implement [`Uniform`] range
/// sampling for a custom type.
///
/// Implementation of [`sample_single`] is optional, and is only useful when
/// the implementation can be faster than `Self::new(low, high).sample(rng)`.
///
/// [module documentation]: crate::distr::uniform
/// [`sample_single`]: UniformSampler::sample_single
pub trait UniformSampler: Sized {
    /// The type sampled by this implementation.
    type X;

    /// Construct self, with inclusive lower bound and exclusive upper bound `[low, high)`.
    ///
    /// For discrete types (e.g. integers), samples will always be strictly less
    /// than `high`. For (approximations of) continuous types (e.g. `f32`, `f64`),
    /// samples may equal `high` due to loss of precision but may not be
    /// greater than `high`.
    ///
    /// Usually users should not call this directly but prefer to use
    /// [`Uniform::new`].
    fn new<B1, B2>(low: B1, high: B2) -> Result<Self, Error>
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized;

    /// Construct self, with inclusive bounds `[low, high]`.
    ///
    /// Usually users should not call this directly but prefer to use
    /// [`Uniform::new_inclusive`].
    fn new_inclusive<B1, B2>(low: B1, high: B2) -> Result<Self, Error>
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized;

    /// Sample a value.
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X;

    /// Sample a single value uniformly from a range with inclusive lower bound
    /// and exclusive upper bound `[low, high)`.
    ///
    /// For discrete types (e.g. integers), samples will always be strictly less
    /// than `high`. For (approximations of) continuous types (e.g. `f32`, `f64`),
    /// samples may equal `high` due to loss of precision but may not be
    /// greater than `high`.
    ///
    /// By default this is implemented using
    /// `UniformSampler::new(low, high).sample(rng)`. However, for some types
    /// more optimal implementations for single usage may be provided via this
    /// method (which is the case for integers and floats).
    /// Results may not be identical.
    ///
    /// Note that to use this method in a generic context, the type needs to be
    /// retrieved via `SampleUniform::Sampler` as follows:
    /// ```
    /// use rand::distr::uniform::{SampleUniform, UniformSampler};
    /// # #[allow(unused)]
    /// fn sample_from_range<T: SampleUniform>(lb: T, ub: T) -> T {
    ///     let mut rng = rand::rng();
    ///     <T as SampleUniform>::Sampler::sample_single(lb, ub, &mut rng).unwrap()
    /// }
    /// ```
    fn sample_single<R: Rng + ?Sized, B1, B2>(
        low: B1,
        high: B2,
        rng: &mut R,
    ) -> Result<Self::X, Error>
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        let uniform: Self = UniformSampler::new(low, high)?;
        Ok(uniform.sample(rng))
    }

    /// Sample a single value uniformly from a range with inclusive lower bound
    /// and inclusive upper bound `[low, high]`.
    ///
    /// By default this is implemented using
    /// `UniformSampler::new_inclusive(low, high).sample(rng)`. However, for
    /// some types more optimal implementations for single usage may be provided
    /// via this method.
    /// Results may not be identical.
    fn sample_single_inclusive<R: Rng + ?Sized, B1, B2>(
        low: B1,
        high: B2,
        rng: &mut R,
    ) -> Result<Self::X, Error>
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        let uniform: Self = UniformSampler::new_inclusive(low, high)?;
        Ok(uniform.sample(rng))
    }
}

impl<X: SampleUniform> TryFrom<Range<X>> for Uniform<X> {
    type Error = Error;

    fn try_from(r: Range<X>) -> Result<Uniform<X>, Error> {
        Uniform::new(r.start, r.end)
    }
}

impl<X: SampleUniform> TryFrom<RangeInclusive<X>> for Uniform<X> {
    type Error = Error;

    fn try_from(r: ::core::ops::RangeInclusive<X>) -> Result<Uniform<X>, Error> {
        Uniform::new_inclusive(r.start(), r.end())
    }
}

/// Helper trait similar to [`Borrow`] but implemented
/// only for [`SampleUniform`] and references to [`SampleUniform`]
/// in order to resolve ambiguity issues.
///
/// [`Borrow`]: std::borrow::Borrow
pub trait SampleBorrow<Borrowed> {
    /// Immutably borrows from an owned value. See [`Borrow::borrow`]
    ///
    /// [`Borrow::borrow`]: std::borrow::Borrow::borrow
    fn borrow(&self) -> &Borrowed;
}
impl<Borrowed> SampleBorrow<Borrowed> for Borrowed
where
    Borrowed: SampleUniform,
{
    #[inline(always)]
    fn borrow(&self) -> &Borrowed {
        self
    }
}
impl<Borrowed> SampleBorrow<Borrowed> for &Borrowed
where
    Borrowed: SampleUniform,
{
    #[inline(always)]
    fn borrow(&self) -> &Borrowed {
        self
    }
}

/// Range that supports generating a single sample efficiently.
///
/// Any type implementing this trait can be used to specify the sampled range
/// for `Rng::random_range`.
pub trait SampleRange<T> {
    /// Generate a sample from the given range.
    fn sample_single<R: RngCore + ?Sized>(self, rng: &mut R) -> Result<T, Error>;

    /// Check whether the range is empty.
    fn is_empty(&self) -> bool;
}

impl<T: SampleUniform + PartialOrd> SampleRange<T> for Range<T> {
    #[inline]
    fn sample_single<R: RngCore + ?Sized>(self, rng: &mut R) -> Result<T, Error> {
        T::Sampler::sample_single(self.start, self.end, rng)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        !(self.start < self.end)
    }
}

impl<T: SampleUniform + PartialOrd> SampleRange<T> for RangeInclusive<T> {
    #[inline]
    fn sample_single<R: RngCore + ?Sized>(self, rng: &mut R) -> Result<T, Error> {
        T::Sampler::sample_single_inclusive(self.start(), self.end(), rng)
    }

    #[inline]
    fn is_empty(&self) -> bool {
        !(self.start() <= self.end())
    }
}

macro_rules! impl_sample_range_u {
    ($t:ty) => {
        impl SampleRange<$t> for RangeTo<$t> {
            #[inline]
            fn sample_single<R: RngCore + ?Sized>(self, rng: &mut R) -> Result<$t, Error> {
                <$t as SampleUniform>::Sampler::sample_single(0, self.end, rng)
            }

            #[inline]
            fn is_empty(&self) -> bool {
                0 == self.end
            }
        }

        impl SampleRange<$t> for RangeToInclusive<$t> {
            #[inline]
            fn sample_single<R: RngCore + ?Sized>(self, rng: &mut R) -> Result<$t, Error> {
                <$t as SampleUniform>::Sampler::sample_single_inclusive(0, self.end, rng)
            }

            #[inline]
            fn is_empty(&self) -> bool {
                false
            }
        }
    };
}

impl_sample_range_u!(u8);
impl_sample_range_u!(u16);
impl_sample_range_u!(u32);
impl_sample_range_u!(u64);
impl_sample_range_u!(u128);
impl_sample_range_u!(usize);

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;

    #[test]
    #[cfg(feature = "serde")]
    fn test_uniform_serialization() {
        let unit_box: Uniform<i32> = Uniform::new(-1, 1).unwrap();
        let de_unit_box: Uniform<i32> =
            bincode::deserialize(&bincode::serialize(&unit_box).unwrap()).unwrap();
        assert_eq!(unit_box.0, de_unit_box.0);

        let unit_box: Uniform<f32> = Uniform::new(-1., 1.).unwrap();
        let de_unit_box: Uniform<f32> =
            bincode::deserialize(&bincode::serialize(&unit_box).unwrap()).unwrap();
        assert_eq!(unit_box.0, de_unit_box.0);
    }

    #[test]
    fn test_custom_uniform() {
        use crate::distr::uniform::{SampleBorrow, SampleUniform, UniformFloat, UniformSampler};
        #[derive(Clone, Copy, PartialEq, PartialOrd)]
        struct MyF32 {
            x: f32,
        }
        #[derive(Clone, Copy, Debug)]
        struct UniformMyF32(UniformFloat<f32>);
        impl UniformSampler for UniformMyF32 {
            type X = MyF32;

            fn new<B1, B2>(low: B1, high: B2) -> Result<Self, Error>
            where
                B1: SampleBorrow<Self::X> + Sized,
                B2: SampleBorrow<Self::X> + Sized,
            {
                UniformFloat::<f32>::new(low.borrow().x, high.borrow().x).map(UniformMyF32)
            }

            fn new_inclusive<B1, B2>(low: B1, high: B2) -> Result<Self, Error>
            where
                B1: SampleBorrow<Self::X> + Sized,
                B2: SampleBorrow<Self::X> + Sized,
            {
                UniformSampler::new(low, high)
            }

            fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
                MyF32 {
                    x: self.0.sample(rng),
                }
            }
        }
        impl SampleUniform for MyF32 {
            type Sampler = UniformMyF32;
        }

        let (low, high) = (MyF32 { x: 17.0f32 }, MyF32 { x: 22.0f32 });
        let uniform = Uniform::new(low, high).unwrap();
        let mut rng = crate::test::rng(804);
        for _ in 0..100 {
            let x: MyF32 = rng.sample(uniform);
            assert!(low <= x && x < high);
        }
    }

    #[test]
    fn value_stability() {
        fn test_samples<T: SampleUniform + Copy + fmt::Debug + PartialEq>(
            lb: T,
            ub: T,
            expected_single: &[T],
            expected_multiple: &[T],
        ) where
            Uniform<T>: Distribution<T>,
        {
            let mut rng = crate::test::rng(897);
            let mut buf = [lb; 3];

            for x in &mut buf {
                *x = T::Sampler::sample_single(lb, ub, &mut rng).unwrap();
            }
            assert_eq!(&buf, expected_single);

            let distr = Uniform::new(lb, ub).unwrap();
            for x in &mut buf {
                *x = rng.sample(&distr);
            }
            assert_eq!(&buf, expected_multiple);
        }

        test_samples(
            0f32,
            1e-2f32,
            &[0.0003070104, 0.0026630748, 0.00979833],
            &[0.008194133, 0.00398172, 0.007428536],
        );
        test_samples(
            -1e10f64,
            1e10f64,
            &[-4673848682.871551, 6388267422.932352, 4857075081.198343],
            &[1173375212.1808167, 1917642852.109581, 2365076174.3153973],
        );

        test_samples(
            Duration::new(2, 0),
            Duration::new(4, 0),
            &[
                Duration::new(2, 532615131),
                Duration::new(3, 638826742),
                Duration::new(3, 485707508),
            ],
            &[
                Duration::new(3, 117337521),
                Duration::new(3, 191764285),
                Duration::new(3, 236507617),
            ],
        );
    }

    #[test]
    fn uniform_distributions_can_be_compared() {
        assert_eq!(
            Uniform::new(1.0, 2.0).unwrap(),
            Uniform::new(1.0, 2.0).unwrap()
        );

        // To cover UniformInt
        assert_eq!(
            Uniform::new(1_u32, 2_u32).unwrap(),
            Uniform::new(1_u32, 2_u32).unwrap()
        );
    }
}
