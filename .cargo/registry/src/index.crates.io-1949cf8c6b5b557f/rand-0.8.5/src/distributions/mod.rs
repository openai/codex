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
//! functionality of the [`Rng`] trait, e.g. [`Rng::gen`] and of course
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
//! # The `Standard` distribution
//!
//! The [`Standard`] distribution is important to mention. This is the
//! distribution used by [`Rng::gen`] and represents the "default" way to
//! produce a random value for many different types, including most primitive
//! types, tuples, arrays, and a few derived types. See the documentation of
//! [`Standard`] for more details.
//!
//! Implementing `Distribution<T>` for [`Standard`] for user types `T` makes it
//! possible to generate type `T` with [`Rng::gen`], and by extension also
//! with the [`random`] function.
//!
//! ## Random characters
//!
//! [`Alphanumeric`] is a simple distribution to sample random letters and
//! numbers of the `char` type; in contrast [`Standard`] may sample any valid
//! `char`.
//!
//!
//! # Uniform numeric ranges
//!
//! The [`Uniform`] distribution is more flexible than [`Standard`], but also
//! more specialised: it supports fewer target types, but allows the sample
//! space to be specified as an arbitrary range within its target type `T`.
//! Both [`Standard`] and [`Uniform`] are in some sense uniform distributions.
//!
//! Values may be sampled from this distribution using [`Rng::sample(Range)`] or
//! by creating a distribution object with [`Uniform::new`],
//! [`Uniform::new_inclusive`] or `From<Range>`. When the range limits are not
//! known at compile time it is typically faster to reuse an existing
//! `Uniform` object than to call [`Rng::sample(Range)`].
//!
//! User types `T` may also implement `Distribution<T>` for [`Uniform`],
//! although this is less straightforward than for [`Standard`] (see the
//! documentation in the [`uniform`] module). Doing so enables generation of
//! values of type `T` with  [`Rng::sample(Range)`].
//!
//! ## Open and half-open ranges
//!
//! There are surprisingly many ways to uniformly generate random floats. A
//! range between 0 and 1 is standard, but the exact bounds (open vs closed)
//! and accuracy differ. In addition to the [`Standard`] distribution Rand offers
//! [`Open01`] and [`OpenClosed01`]. See "Floating point implementation" section of
//! [`Standard`] documentation for more details.
//!
//! # Non-uniform sampling
//!
//! Sampling a simple true/false outcome with a given probability has a name:
//! the [`Bernoulli`] distribution (this is used by [`Rng::gen_bool`]).
//!
//! For weighted sampling from a sequence of discrete values, use the
//! [`WeightedIndex`] distribution.
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
mod slice;
mod utils;
#[cfg(feature = "alloc")]
mod weighted_index;

#[doc(hidden)]
pub mod hidden_export {
    pub use super::float::IntoFloat; // used by rand_distr
}
pub mod uniform;
#[deprecated(
    since = "0.8.0",
    note = "use rand::distributions::{WeightedIndex, WeightedError} instead"
)]
#[cfg(feature = "alloc")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "alloc")))]
pub mod weighted;

pub use self::bernoulli::{Bernoulli, BernoulliError};
pub use self::distribution::{Distribution, DistIter, DistMap};
#[cfg(feature = "alloc")]
pub use self::distribution::DistString;
pub use self::float::{Open01, OpenClosed01};
pub use self::other::Alphanumeric;
pub use self::slice::Slice;
#[doc(inline)]
pub use self::uniform::Uniform;
#[cfg(feature = "alloc")]
pub use self::weighted_index::{WeightedError, WeightedIndex};

#[allow(unused)]
use crate::Rng;

/// A generic random value distribution, implemented for many primitive types.
/// Usually generates values with a numerically uniform distribution, and with a
/// range appropriate to the type.
///
/// ## Provided implementations
///
/// Assuming the provided `Rng` is well-behaved, these implementations
/// generate values with the following ranges and distributions:
///
/// * Integers (`i32`, `u32`, `isize`, `usize`, etc.): Uniformly distributed
///   over all values of the type.
/// * `char`: Uniformly distributed over all Unicode scalar values, i.e. all
///   code points in the range `0...0x10_FFFF`, except for the range
///   `0xD800...0xDFFF` (the surrogate code points). This includes
///   unassigned/reserved code points.
/// * `bool`: Generates `false` or `true`, each with probability 0.5.
/// * Floating point types (`f32` and `f64`): Uniformly distributed in the
///   half-open range `[0, 1)`. See notes below.
/// * Wrapping integers (`Wrapping<T>`), besides the type identical to their
///   normal integer variants.
///
/// The `Standard` distribution also supports generation of the following
/// compound types where all component types are supported:
///
/// *   Tuples (up to 12 elements): each element is generated sequentially.
/// *   Arrays (up to 32 elements): each element is generated sequentially;
///     see also [`Rng::fill`] which supports arbitrary array length for integer
///     and float types and tends to be faster for `u32` and smaller types.
///     When using `rustc` ≥ 1.51, enable the `min_const_gen` feature to support
///     arrays larger than 32 elements.
///     Note that [`Rng::fill`] and `Standard`'s array support are *not* equivalent:
///     the former is optimised for integer types (using fewer RNG calls for
///     element types smaller than the RNG word size), while the latter supports
///     any element type supported by `Standard`.
/// *   `Option<T>` first generates a `bool`, and if true generates and returns
///     `Some(value)` where `value: T`, otherwise returning `None`.
///
/// ## Custom implementations
///
/// The [`Standard`] distribution may be implemented for user types as follows:
///
/// ```
/// # #![allow(dead_code)]
/// use rand::Rng;
/// use rand::distributions::{Distribution, Standard};
///
/// struct MyF32 {
///     x: f32,
/// }
///
/// impl Distribution<MyF32> for Standard {
///     fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> MyF32 {
///         MyF32 { x: rng.gen() }
///     }
/// }
/// ```
///
/// ## Example usage
/// ```
/// use rand::prelude::*;
/// use rand::distributions::Standard;
///
/// let val: f32 = StdRng::from_entropy().sample(Standard);
/// println!("f32 from [0, 1): {}", val);
/// ```
///
/// # Floating point implementation
/// The floating point implementations for `Standard` generate a random value in
/// the half-open interval `[0, 1)`, i.e. including 0 but not 1.
///
/// All values that can be generated are of the form `n * ε/2`. For `f32`
/// the 24 most significant random bits of a `u32` are used and for `f64` the
/// 53 most significant bits of a `u64` are used. The conversion uses the
/// multiplicative method: `(rng.gen::<$uty>() >> N) as $ty * (ε/2)`.
///
/// See also: [`Open01`] which samples from `(0, 1)`, [`OpenClosed01`] which
/// samples from `(0, 1]` and `Rng::gen_range(0..1)` which also samples from
/// `[0, 1)`. Note that `Open01` uses transmute-based methods which yield 1 bit
/// less precision but may perform faster on some architectures (on modern Intel
/// CPUs all methods have approximately equal performance).
///
/// [`Uniform`]: uniform::Uniform
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde1", derive(serde::Serialize, serde::Deserialize))]
pub struct Standard;
