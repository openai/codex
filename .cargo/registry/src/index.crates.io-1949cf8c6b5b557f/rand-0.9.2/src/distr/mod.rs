// Copyright 2018 Developers of the Rand project.
// Copyright 2013-2017 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Generating random samples from probability distributions
//!
//! This module is the home of the [`Distribution`] trait and several of its
//! implementations. It is the workhorse behind some of the convenient
//! functionality of the [`Rng`] trait, e.g. [`Rng::random`] and of course
//! [`Rng::sample`].
//!
//! Abstractly, a [probability distribution] describes the probability of
//! occurrence of each value in its sample space.
//!
//! More concretely, an implementation of `Distribution<T>` for type `X` is an
//! algorithm for choosing values from the sample space (a subset of `T`)
//! according to the distribution `X` represents, using an external source of
//! randomness (an RNG supplied to the `sample` function).
//!
//! A type `X` may implement `Distribution<T>` for multiple types `T`.
//! Any type implementing [`Distribution`] is stateless (i.e. immutable),
//! but it may have internal parameters set at construction time (for example,
//! [`Uniform`] allows specification of its sample space as a range within `T`).
//!
//!
//! # The Standard Uniform distribution
//!
//! The [`StandardUniform`] distribution is important to mention. This is the
//! distribution used by [`Rng::random`] and represents the "default" way to
//! produce a random value for many different types, including most primitive
//! types, tuples, arrays, and a few derived types. See the documentation of
//! [`StandardUniform`] for more details.
//!
//! Implementing [`Distribution<T>`] for [`StandardUniform`] for user types `T` makes it
//! possible to generate type `T` with [`Rng::random`], and by extension also
//! with the [`random`] function.
//!
//! ## Other standard uniform distributions
//!
//! [`Alphanumeric`] is a simple distribution to sample random letters and
//! numbers of the `char` type; in contrast [`StandardUniform`] may sample any valid
//! `char`.
//!
//! There's also an [`Alphabetic`] distribution which acts similarly to [`Alphanumeric`] but
//! doesn't include digits.
//!
//! For floats (`f32`, `f64`), [`StandardUniform`] samples from `[0, 1)`. Also
//! provided are [`Open01`] (samples from `(0, 1)`) and [`OpenClosed01`]
//! (samples from `(0, 1]`). No option is provided to sample from `[0, 1]`; it
//! is suggested to use one of the above half-open ranges since the failure to
//! sample a value which would have a low chance of being sampled anyway is
//! rarely an issue in practice.
//!
//! # Parameterized Uniform distributions
//!
//! The [`Uniform`] distribution provides uniform sampling over a specified
//! range on a subset of the types supported by the above distributions.
//!
//! Implementations support single-value-sampling via
//! [`Rng::random_range(Range)`](Rng::random_range).
//! Where a fixed (non-`const`) range will be sampled many times, it is likely
//! faster to pre-construct a [`Distribution`] object using
//! [`Uniform::new`], [`Uniform::new_inclusive`] or `From<Range>`.
//!
//! # Non-uniform sampling
//!
//! Sampling a simple true/false outcome with a given probability has a name:
//! the [`Bernoulli`] distribution (this is used by [`Rng::random_bool`]).
//!
//! For weighted sampling of discrete values see the [`weighted`] module.
//!
//! This crate no longer includes other non-uniform distributions; instead
//! it is recommended that you use either [`rand_distr`] or [`statrs`].
//!
//!
//! [probability distribution]: https://en.wikipedia.org/wiki/Probability_distribution
//! [`rand_distr`]: https://crates.io/crates/rand_distr
//! [`statrs`]: https://crates.io/crates/statrs

//! [`random`]: crate::random
//! [`rand_distr`]: https://crates.io/crates/rand_distr
//! [`statrs`]: https://crates.io/crates/statrs

mod bernoulli;
mod distribution;
mod float;
mod integer;
mod other;
mod utils;

#[doc(hidden)]
pub mod hidden_export {
    pub use super::float::IntoFloat; // used by rand_distr
}
pub mod slice;
pub mod uniform;
#[cfg(feature = "alloc")]
pub mod weighted;

pub use self::bernoulli::{Bernoulli, BernoulliError};
#[cfg(feature = "alloc")]
pub use self::distribution::SampleString;
pub use self::distribution::{Distribution, Iter, Map};
pub use self::float::{Open01, OpenClosed01};
pub use self::other::{Alphabetic, Alphanumeric};
#[doc(inline)]
pub use self::uniform::Uniform;

#[allow(unused)]
use crate::Rng;

/// The Standard Uniform distribution
///
/// This [`Distribution`] is the *standard* parameterization of [`Uniform`]. Bounds
/// are selected according to the output type.
///
/// Assuming the provided `Rng` is well-behaved, these implementations
/// generate values with the following ranges and distributions:
///
/// * Integers (`i8`, `i32`, `u64`, etc.) are uniformly distributed
///   over the whole range of the type (thus each possible value may be sampled
///   with equal probability).
/// * `char` is uniformly distributed over all Unicode scalar values, i.e. all
///   code points in the range `0...0x10_FFFF`, except for the range
///   `0xD800...0xDFFF` (the surrogate code points). This includes
///   unassigned/reserved code points.
///   For some uses, the [`Alphanumeric`] or [`Alphabetic`] distribution will be more
///   appropriate.
/// * `bool` samples `false` or `true`, each with probability 0.5.
/// * Floating point types (`f32` and `f64`) are uniformly distributed in the
///   half-open range `[0, 1)`. See also the [notes below](#floating-point-implementation).
/// * Wrapping integers ([`Wrapping<T>`]), besides the type identical to their
///   normal integer variants.
/// * Non-zero integers ([`NonZeroU8`]), which are like their normal integer
///   variants but cannot sample zero.
///
/// The `StandardUniform` distribution also supports generation of the following
/// compound types where all component types are supported:
///
/// * Tuples (up to 12 elements): each element is sampled sequentially and
///   independently (thus, assuming a well-behaved RNG, there is no correlation
///   between elements).
/// * Arrays `[T; n]` where `T` is supported. Each element is sampled
///   sequentially and independently. Note that for small `T` this usually
///   results in the RNG discarding random bits; see also [`Rng::fill`] which
///   offers a more efficient approach to filling an array of integer types
///   with random data.
/// * SIMD types (requires [`simd_support`] feature) like x86's [`__m128i`]
///   and `std::simd`'s [`u32x4`], [`f32x4`] and [`mask32x4`] types are
///   effectively arrays of integer or floating-point types. Each lane is
///   sampled independently, potentially with more efficient random-bit-usage
///   (and a different resulting value) than would be achieved with sequential
///   sampling (as with the array types above).
///
/// ## Custom implementations
///
/// The [`StandardUniform`] distribution may be implemented for user types as follows:
///
/// ```
/// # #![allow(dead_code)]
/// use rand::Rng;
/// use rand::distr::{Distribution, StandardUniform};
///
/// struct MyF32 {
///     x: f32,
/// }
///
/// impl Distribution<MyF32> for StandardUniform {
///     fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> MyF32 {
///         MyF32 { x: rng.random() }
///     }
/// }
/// ```
///
/// ## Example usage
/// ```
/// use rand::prelude::*;
/// use rand::distr::StandardUniform;
///
/// let val: f32 = rand::rng().sample(StandardUniform);
/// println!("f32 from [0, 1): {}", val);
/// ```
///
/// # Floating point implementation
/// The floating point implementations for `StandardUniform` generate a random value in
/// the half-open interval `[0, 1)`, i.e. including 0 but not 1.
///
/// All values that can be generated are of the form `n * ε/2`. For `f32`
/// the 24 most significant random bits of a `u32` are used and for `f64` the
/// 53 most significant bits of a `u64` are used. The conversion uses the
/// multiplicative method: `(rng.gen::<$uty>() >> N) as $ty * (ε/2)`.
///
/// See also: [`Open01`] which samples from `(0, 1)`, [`OpenClosed01`] which
/// samples from `(0, 1]` and `Rng::random_range(0..1)` which also samples from
/// `[0, 1)`. Note that `Open01` uses transmute-based methods which yield 1 bit
/// less precision but may perform faster on some architectures (on modern Intel
/// CPUs all methods have approximately equal performance).
///
/// [`Uniform`]: uniform::Uniform
/// [`Wrapping<T>`]: std::num::Wrapping
/// [`NonZeroU8`]: std::num::NonZeroU8
/// [`__m128i`]: https://doc.rust-lang.org/core/arch/x86/struct.__m128i.html
/// [`u32x4`]: std::simd::u32x4
/// [`f32x4`]: std::simd::f32x4
/// [`mask32x4`]: std::simd::mask32x4
/// [`simd_support`]: https://github.com/rust-random/rand#crate-features
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StandardUniform;
