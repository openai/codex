//! A crate that provides support for half-precision 16-bit floating point types.
//!
//! This crate provides the [`struct@f16`] type, which is an implementation of the IEEE 754-2008 standard
//! [`binary16`] a.k.a "half" floating point type. This 16-bit floating point type is intended for
//! efficient storage where the full range and precision of a larger floating point value is not
//! required. This is especially useful for image storage formats.
//!
//! This crate also provides a [`struct@bf16`] type, an alternative 16-bit floating point format. The
//! [`bfloat16`] format is a truncated IEEE 754 standard `binary32` float that preserves the
//! exponent to allow the same range as [`f32`] but with only 8 bits of precision (instead of 11
//! bits for [`struct@f16`]). See the [`struct@bf16`] type for details.
//!
//! Because [`struct@f16`] and [`struct@bf16`] are primarily for efficient storage, floating point operations such
//! as addition, multiplication, etc. are not always implemented by hardware. When hardware does not
//! support these operations, this crate emulates them by converting the value to
//! [`f32`] before performing the operation and then back afterward.
//!
//! Note that conversion from [`f32`]/[`f64`] to both [`struct@f16`] and [`struct@bf16`] are lossy operations, and
//! just as converting a [`f64`] to [`f32`] is lossy and does not have `Into`/`From` trait
//! implementations, so too do these smaller types not have those trait implementations either.
//! Instead, use `from_f32`/`from_f64` functions for the types in this crate. If you don't care
//! about lossy conversions and need trait conversions, use the appropriate [`num-traits`]
//! traits that are implemented.
//!
//! This crate also provides a [`slice`][mod@slice] module for zero-copy in-place conversions of
//! [`u16`] slices to both [`struct@f16`] and [`struct@bf16`], as well as efficient vectorized conversions of
//! larger buffers of floating point values to and from these half formats.
//!
//! The crate supports `#[no_std]` when the `std` cargo feature is not enabled, so can be used in
//! embedded environments without using the Rust [`std`] library. The `std` feature enables support
//! for the standard library and is enabled by default, see the [Cargo Features](#cargo-features)
//! section below.
//!
//! A [`prelude`] module is provided for easy importing of available utility traits.
//!
//! # Serialization
//!
//! When the `serde` feature is enabled, [`struct@f16`] and [`struct@bf16`] will be serialized as a newtype of
//! [`u16`] by default. In binary formats this is ideal, as it will generally use just two bytes for
//! storage. For string formats like JSON, however, this isn't as useful, and due to design
//! limitations of serde, it's not possible for the default `Serialize` implementation to support
//! different serialization for different formats.
//!
//! Instead, it's up to the containter type of the floats to control how it is serialized. This can
//! easily be controlled when using the derive macros using `#[serde(serialize_with="")]`
//! attributes. For both [`struct@f16`] and [`struct@bf16`] a `serialize_as_f32` and `serialize_as_string` are
//! provided for use with this attribute.
//!
//! Deserialization of both float types supports deserializing from the default serialization,
//! strings, and `f32`/`f64` values, so no additional work is required.
//!
//! # Hardware support
//!
//! Hardware support for these conversions and arithmetic will be used
//! whenever hardware support is available—either through instrinsics or targeted assembly—although
//! a nightly Rust toolchain may be required for some hardware. When hardware supports it the
//! functions and traits in the [`slice`][mod@slice] and [`vec`] modules will also use vectorized
//! SIMD intructions for increased efficiency.
//!
//! The following list details hardware support for floating point types in this crate. When using
//! `std` cargo feature, runtime CPU target detection will be used. To get the most performance
//! benefits, compile for specific CPU features which avoids the runtime overhead and works in a
//! `no_std` environment.
//!
//! | Architecture | CPU Target Feature | Notes |
//! | ------------ | ------------------ | ----- |
//! | `x86`/`x86_64` | `f16c` | This supports conversion to/from [`struct@f16`] only (including vector SIMD) and does not support any [`struct@bf16`] or arithmetic operations. |
//! | `aarch64` | `fp16` | This supports all operations on [`struct@f16`] only. |
//!
//! # Cargo Features
//!
//! This crate supports a number of optional cargo features. None of these features are enabled by
//! default, even `std`.
//!
//! - **`alloc`** — Enable use of the [`alloc`] crate when not using the `std` library.
//!
//!   Among other functions, this enables the [`vec`] module, which contains zero-copy
//!   conversions for the [`Vec`] type. This allows fast conversion between raw `Vec<u16>` bits and
//!   `Vec<f16>` or `Vec<bf16>` arrays, and vice versa.
//!
//! - **`std`** — Enable features that depend on the Rust [`std`] library. This also enables the
//!   `alloc` feature automatically.
//!
//!   Enabling the `std` feature enables runtime CPU feature detection of hardware support.
//!   Without this feature detection, harware is only used when compiler target supports them.
//!
//! - **`serde`** — Adds support for the [`serde`] crate by implementing [`Serialize`] and
//!   [`Deserialize`] traits for both [`struct@f16`] and [`struct@bf16`].
//!
//! - **`num-traits`** — Adds support for the [`num-traits`] crate by implementing [`ToPrimitive`],
//!   [`FromPrimitive`], [`ToBytes`], `FromBytes`, [`AsPrimitive`], [`Num`], [`Float`],
//!   [`FloatCore`], and [`Bounded`] traits for both [`struct@f16`] and [`struct@bf16`].
//!
//! - **`bytemuck`** — Adds support for the [`bytemuck`] crate by implementing [`Zeroable`] and
//!   [`Pod`] traits for both [`struct@f16`] and [`struct@bf16`].
//!
//! - **`zerocopy`** — Adds support for the [`zerocopy`] crate by implementing [`IntoBytes`] and
//!   [`FromBytes`] traits for both [`struct@f16`] and [`struct@bf16`].
//!
//! - **`rand_distr`** — Adds support for the [`rand_distr`] crate by implementing [`Distribution`]
//!   and other traits for both [`struct@f16`] and [`struct@bf16`].
//!
//! - **`rkyv`** -- Enable zero-copy deserializtion with [`rkyv`] crate.
//!
//! - **`aribtrary`** -- Enable fuzzing support with [`arbitrary`] crate by implementing
//!   [`Arbitrary`] trait.
//!
//! [`alloc`]: https://doc.rust-lang.org/alloc/
//! [`std`]: https://doc.rust-lang.org/std/
//! [`binary16`]: https://en.wikipedia.org/wiki/Half-precision_floating-point_format
//! [`bfloat16`]: https://en.wikipedia.org/wiki/Bfloat16_floating-point_format
//! [`serde`]: https://crates.io/crates/serde
//! [`bytemuck`]: https://crates.io/crates/bytemuck
//! [`num-traits`]: https://crates.io/crates/num-traits
//! [`zerocopy`]: https://crates.io/crates/zerocopy
//! [`rand_distr`]: https://crates.io/crates/rand_distr
//! [`rkyv`]: (https://crates.io/crates/rkyv)
//! [`arbitrary`]: (https://crates.io/crates/arbitrary)
#![cfg_attr(
    feature = "alloc",
    doc = "
[`vec`]: mod@vec"
)]
#![cfg_attr(
    not(feature = "alloc"),
    doc = "
[`vec`]: #
[`Vec`]: https://docs.rust-lang.org/stable/alloc/vec/struct.Vec.html"
)]
#![cfg_attr(
    feature = "serde",
    doc = "
[`Serialize`]: serde::Serialize
[`Deserialize`]: serde::Deserialize"
)]
#![cfg_attr(
    not(feature = "serde"),
    doc = "
[`Serialize`]: https://docs.rs/serde/*/serde/trait.Serialize.html
[`Deserialize`]: https://docs.rs/serde/*/serde/trait.Deserialize.html"
)]
#![cfg_attr(
    feature = "num-traits",
    doc = "
[`ToPrimitive`]: ::num_traits::ToPrimitive
[`FromPrimitive`]: ::num_traits::FromPrimitive
[`ToBytes`]: ::num_traits::ToBytes
[`AsPrimitive`]: ::num_traits::AsPrimitive
[`Num`]: ::num_traits::Num
[`Float`]: ::num_traits::Float
[`FloatCore`]: ::num_traits::float::FloatCore
[`Bounded`]: ::num_traits::Bounded"
)]
#![cfg_attr(
    not(feature = "num-traits"),
    doc = "
[`ToPrimitive`]: https://docs.rs/num-traits/*/num_traits/cast/trait.ToPrimitive.html
[`FromPrimitive`]: https://docs.rs/num-traits/*/num_traits/cast/trait.FromPrimitive.html
[`ToBytes`]: https://docs.rs/num-traits/*/num_traits/ops/bytes/trait.ToBytes.html
[`AsPrimitive`]: https://docs.rs/num-traits/*/num_traits/cast/trait.AsPrimitive.html
[`Num`]: https://docs.rs/num-traits/*/num_traits/trait.Num.html
[`Float`]: https://docs.rs/num-traits/*/num_traits/float/trait.Float.html
[`FloatCore`]: https://docs.rs/num-traits/*/num_traits/float/trait.FloatCore.html
[`Bounded`]: https://docs.rs/num-traits/*/num_traits/bounds/trait.Bounded.html"
)]
#![cfg_attr(
    feature = "bytemuck",
    doc = "
[`Zeroable`]: bytemuck::Zeroable
[`Pod`]: bytemuck::Pod"
)]
#![cfg_attr(
    not(feature = "bytemuck"),
    doc = "
[`Zeroable`]: https://docs.rs/bytemuck/*/bytemuck/trait.Zeroable.html
[`Pod`]: https://docs.rs/bytemuck/*bytemuck/trait.Pod.html"
)]
#![cfg_attr(
    feature = "zerocopy",
    doc = "
[`IntoBytes`]: zerocopy::IntoBytes
[`FromBytes`]: zerocopy::FromBytes"
)]
#![cfg_attr(
    not(feature = "zerocopy"),
    doc = "
[`IntoBytes`]: https://docs.rs/zerocopy/*/zerocopy/trait.IntoBytes.html
[`FromBytes`]: https://docs.rs/zerocopy/*/zerocopy/trait.FromBytes.html"
)]
#![cfg_attr(
    feature = "rand_distr",
    doc = "
[`Distribution`]: rand::distr::Distribution"
)]
#![cfg_attr(
    not(feature = "rand_distr"),
    doc = "
[`Distribution`]: https://docs.rs/rand/*/rand/distr/trait.Distribution.html"
)]
#![cfg_attr(
    feature = "arbitrary",
    doc = "
[`Arbitrary`]: arbitrary::Arbitrary"
)]
#![cfg_attr(
    not(feature = "arbitrary"),
    doc = "
[`Arbitrary`]: https://docs.rs/arbitrary/*/arbitrary/trait.Arbitrary.html"
)]
#![warn(
    missing_docs,
    missing_copy_implementations,
    trivial_numeric_casts,
    future_incompatible
)]
#![cfg_attr(not(target_arch = "spirv"), warn(missing_debug_implementations))]
#![allow(clippy::verbose_bit_mask, clippy::cast_lossless, unexpected_cfgs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![doc(html_root_url = "https://docs.rs/half/2.6.0")]
#![doc(test(attr(deny(warnings), allow(unused))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

mod bfloat;
mod binary16;
mod leading_zeros;
#[cfg(feature = "num-traits")]
mod num_traits;

#[cfg(not(target_arch = "spirv"))]
pub mod slice;
#[cfg(feature = "alloc")]
pub mod vec;

pub use bfloat::bf16;
pub use binary16::f16;

#[cfg(feature = "rand_distr")]
mod rand_distr;

/// A collection of the most used items and traits in this crate for easy importing.
///
/// # Examples
///
/// ```rust
/// use half::prelude::*;
/// ```
pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{bf16, f16};

    #[cfg(not(target_arch = "spirv"))]
    #[doc(no_inline)]
    pub use crate::slice::{HalfBitsSliceExt, HalfFloatSliceExt};

    #[cfg(feature = "alloc")]
    #[doc(no_inline)]
    pub use crate::vec::{HalfBitsVecExt, HalfFloatVecExt};
}

// Keep this module private to crate
mod private {
    use crate::{bf16, f16};

    pub trait SealedHalf {}

    impl SealedHalf for f16 {}
    impl SealedHalf for bf16 {}
}
