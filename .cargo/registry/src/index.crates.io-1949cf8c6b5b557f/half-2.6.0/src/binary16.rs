#[cfg(all(feature = "serde", feature = "alloc"))]
#[allow(unused_imports)]
use alloc::string::ToString;
#[cfg(feature = "bytemuck")]
use bytemuck::{Pod, Zeroable};
use core::{
    cmp::Ordering,
    iter::{Product, Sum},
    num::FpCategory,
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign, Sub, SubAssign},
};
#[cfg(not(target_arch = "spirv"))]
use core::{
    fmt::{
        Binary, Debug, Display, Error, Formatter, LowerExp, LowerHex, Octal, UpperExp, UpperHex,
    },
    num::ParseFloatError,
    str::FromStr,
};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "zerocopy")]
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

pub(crate) mod arch;

/// A 16-bit floating point type implementing the IEEE 754-2008 standard [`binary16`] a.k.a "half"
/// format.
///
/// This 16-bit floating point type is intended for efficient storage where the full range and
/// precision of a larger floating point value is not required.
///
/// [`binary16`]: https://en.wikipedia.org/wiki/Half-precision_floating-point_format
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
#[cfg_attr(feature = "rkyv", rkyv(resolver = F16Resolver))]
#[cfg_attr(feature = "bytemuck", derive(Zeroable, Pod))]
#[cfg_attr(
    feature = "zerocopy",
    derive(FromBytes, Immutable, IntoBytes, KnownLayout)
)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct f16(u16);

impl f16 {
    /// Constructs a 16-bit floating point value from the raw bits.
    #[inline]
    #[must_use]
    pub const fn from_bits(bits: u16) -> f16 {
        f16(bits)
    }

    /// Constructs a 16-bit floating point value from a 32-bit floating point value.
    ///
    /// This operation is lossy. If the 32-bit value is to large to fit in 16-bits, ±∞ will result.
    /// NaN values are preserved. 32-bit subnormal values are too tiny to be represented in 16-bits
    /// and result in ±0. Exponents that underflow the minimum 16-bit exponent will result in 16-bit
    /// subnormals or ±0. All other values are truncated and rounded to the nearest representable
    /// 16-bit value.
    #[inline]
    #[must_use]
    pub fn from_f32(value: f32) -> f16 {
        f16(arch::f32_to_f16(value))
    }

    /// Constructs a 16-bit floating point value from a 32-bit floating point value.
    ///
    /// This function is identical to [`from_f32`][Self::from_f32] except it never uses hardware
    /// intrinsics, which allows it to be `const`. [`from_f32`][Self::from_f32] should be preferred
    /// in any non-`const` context.
    ///
    /// This operation is lossy. If the 32-bit value is to large to fit in 16-bits, ±∞ will result.
    /// NaN values are preserved. 32-bit subnormal values are too tiny to be represented in 16-bits
    /// and result in ±0. Exponents that underflow the minimum 16-bit exponent will result in 16-bit
    /// subnormals or ±0. All other values are truncated and rounded to the nearest representable
    /// 16-bit value.
    #[inline]
    #[must_use]
    pub const fn from_f32_const(value: f32) -> f16 {
        f16(arch::f32_to_f16_fallback(value))
    }

    /// Constructs a 16-bit floating point value from a 64-bit floating point value.
    ///
    /// This operation is lossy. If the 64-bit value is to large to fit in 16-bits, ±∞ will result.
    /// NaN values are preserved. 64-bit subnormal values are too tiny to be represented in 16-bits
    /// and result in ±0. Exponents that underflow the minimum 16-bit exponent will result in 16-bit
    /// subnormals or ±0. All other values are truncated and rounded to the nearest representable
    /// 16-bit value.
    #[inline]
    #[must_use]
    pub fn from_f64(value: f64) -> f16 {
        f16(arch::f64_to_f16(value))
    }

    /// Constructs a 16-bit floating point value from a 64-bit floating point value.
    ///
    /// This function is identical to [`from_f64`][Self::from_f64] except it never uses hardware
    /// intrinsics, which allows it to be `const`. [`from_f64`][Self::from_f64] should be preferred
    /// in any non-`const` context.
    ///
    /// This operation is lossy. If the 64-bit value is to large to fit in 16-bits, ±∞ will result.
    /// NaN values are preserved. 64-bit subnormal values are too tiny to be represented in 16-bits
    /// and result in ±0. Exponents that underflow the minimum 16-bit exponent will result in 16-bit
    /// subnormals or ±0. All other values are truncated and rounded to the nearest representable
    /// 16-bit value.
    #[inline]
    #[must_use]
    pub const fn from_f64_const(value: f64) -> f16 {
        f16(arch::f64_to_f16_fallback(value))
    }

    /// Converts a [`struct@f16`] into the underlying bit representation.
    #[inline]
    #[must_use]
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// Returns the memory representation of the underlying bit representation as a byte array in
    /// little-endian byte order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let bytes = f16::from_f32(12.5).to_le_bytes();
    /// assert_eq!(bytes, [0x40, 0x4A]);
    /// ```
    #[inline]
    #[must_use]
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }

    /// Returns the memory representation of the underlying bit representation as a byte array in
    /// big-endian (network) byte order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let bytes = f16::from_f32(12.5).to_be_bytes();
    /// assert_eq!(bytes, [0x4A, 0x40]);
    /// ```
    #[inline]
    #[must_use]
    pub const fn to_be_bytes(self) -> [u8; 2] {
        self.0.to_be_bytes()
    }

    /// Returns the memory representation of the underlying bit representation as a byte array in
    /// native byte order.
    ///
    /// As the target platform's native endianness is used, portable code should use
    /// [`to_be_bytes`][Self::to_be_bytes] or [`to_le_bytes`][Self::to_le_bytes], as appropriate,
    /// instead.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let bytes = f16::from_f32(12.5).to_ne_bytes();
    /// assert_eq!(bytes, if cfg!(target_endian = "big") {
    ///     [0x4A, 0x40]
    /// } else {
    ///     [0x40, 0x4A]
    /// });
    /// ```
    #[inline]
    #[must_use]
    pub const fn to_ne_bytes(self) -> [u8; 2] {
        self.0.to_ne_bytes()
    }

    /// Creates a floating point value from its representation as a byte array in little endian.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let value = f16::from_le_bytes([0x40, 0x4A]);
    /// assert_eq!(value, f16::from_f32(12.5));
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_le_bytes(bytes: [u8; 2]) -> f16 {
        f16::from_bits(u16::from_le_bytes(bytes))
    }

    /// Creates a floating point value from its representation as a byte array in big endian.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let value = f16::from_be_bytes([0x4A, 0x40]);
    /// assert_eq!(value, f16::from_f32(12.5));
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_be_bytes(bytes: [u8; 2]) -> f16 {
        f16::from_bits(u16::from_be_bytes(bytes))
    }

    /// Creates a floating point value from its representation as a byte array in native endian.
    ///
    /// As the target platform's native endianness is used, portable code likely wants to use
    /// [`from_be_bytes`][Self::from_be_bytes] or [`from_le_bytes`][Self::from_le_bytes], as
    /// appropriate instead.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let value = f16::from_ne_bytes(if cfg!(target_endian = "big") {
    ///     [0x4A, 0x40]
    /// } else {
    ///     [0x40, 0x4A]
    /// });
    /// assert_eq!(value, f16::from_f32(12.5));
    /// ```
    #[inline]
    #[must_use]
    pub const fn from_ne_bytes(bytes: [u8; 2]) -> f16 {
        f16::from_bits(u16::from_ne_bytes(bytes))
    }

    /// Converts a [`struct@f16`] value into a `f32` value.
    ///
    /// This conversion is lossless as all 16-bit floating point values can be represented exactly
    /// in 32-bit floating point.
    #[inline]
    #[must_use]
    pub fn to_f32(self) -> f32 {
        arch::f16_to_f32(self.0)
    }

    /// Converts a [`struct@f16`] value into a `f32` value.
    ///
    /// This function is identical to [`to_f32`][Self::to_f32] except it never uses hardware
    /// intrinsics, which allows it to be `const`. [`to_f32`][Self::to_f32] should be preferred
    /// in any non-`const` context.
    ///
    /// This conversion is lossless as all 16-bit floating point values can be represented exactly
    /// in 32-bit floating point.
    #[inline]
    #[must_use]
    pub const fn to_f32_const(self) -> f32 {
        arch::f16_to_f32_fallback(self.0)
    }

    /// Converts a [`struct@f16`] value into a `f64` value.
    ///
    /// This conversion is lossless as all 16-bit floating point values can be represented exactly
    /// in 64-bit floating point.
    #[inline]
    #[must_use]
    pub fn to_f64(self) -> f64 {
        arch::f16_to_f64(self.0)
    }

    /// Converts a [`struct@f16`] value into a `f64` value.
    ///
    /// This function is identical to [`to_f64`][Self::to_f64] except it never uses hardware
    /// intrinsics, which allows it to be `const`. [`to_f64`][Self::to_f64] should be preferred
    /// in any non-`const` context.
    ///
    /// This conversion is lossless as all 16-bit floating point values can be represented exactly
    /// in 64-bit floating point.
    #[inline]
    #[must_use]
    pub const fn to_f64_const(self) -> f64 {
        arch::f16_to_f64_fallback(self.0)
    }

    /// Returns `true` if this value is `NaN` and `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    ///
    /// let nan = f16::NAN;
    /// let f = f16::from_f32(7.0_f32);
    ///
    /// assert!(nan.is_nan());
    /// assert!(!f.is_nan());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_nan(self) -> bool {
        self.0 & 0x7FFFu16 > 0x7C00u16
    }

    /// Returns `true` if this value is ±∞ and `false`.
    /// otherwise.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    ///
    /// let f = f16::from_f32(7.0f32);
    /// let inf = f16::INFINITY;
    /// let neg_inf = f16::NEG_INFINITY;
    /// let nan = f16::NAN;
    ///
    /// assert!(!f.is_infinite());
    /// assert!(!nan.is_infinite());
    ///
    /// assert!(inf.is_infinite());
    /// assert!(neg_inf.is_infinite());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_infinite(self) -> bool {
        self.0 & 0x7FFFu16 == 0x7C00u16
    }

    /// Returns `true` if this number is neither infinite nor `NaN`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    ///
    /// let f = f16::from_f32(7.0f32);
    /// let inf = f16::INFINITY;
    /// let neg_inf = f16::NEG_INFINITY;
    /// let nan = f16::NAN;
    ///
    /// assert!(f.is_finite());
    ///
    /// assert!(!nan.is_finite());
    /// assert!(!inf.is_finite());
    /// assert!(!neg_inf.is_finite());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_finite(self) -> bool {
        self.0 & 0x7C00u16 != 0x7C00u16
    }

    /// Returns `true` if the number is neither zero, infinite, subnormal, or `NaN`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    ///
    /// let min = f16::MIN_POSITIVE;
    /// let max = f16::MAX;
    /// let lower_than_min = f16::from_f32(1.0e-10_f32);
    /// let zero = f16::from_f32(0.0_f32);
    ///
    /// assert!(min.is_normal());
    /// assert!(max.is_normal());
    ///
    /// assert!(!zero.is_normal());
    /// assert!(!f16::NAN.is_normal());
    /// assert!(!f16::INFINITY.is_normal());
    /// // Values between `0` and `min` are Subnormal.
    /// assert!(!lower_than_min.is_normal());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_normal(self) -> bool {
        let exp = self.0 & 0x7C00u16;
        exp != 0x7C00u16 && exp != 0
    }

    /// Returns the floating point category of the number.
    ///
    /// If only one property is going to be tested, it is generally faster to use the specific
    /// predicate instead.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::num::FpCategory;
    /// # use half::prelude::*;
    ///
    /// let num = f16::from_f32(12.4_f32);
    /// let inf = f16::INFINITY;
    ///
    /// assert_eq!(num.classify(), FpCategory::Normal);
    /// assert_eq!(inf.classify(), FpCategory::Infinite);
    /// ```
    #[must_use]
    pub const fn classify(self) -> FpCategory {
        let exp = self.0 & 0x7C00u16;
        let man = self.0 & 0x03FFu16;
        match (exp, man) {
            (0, 0) => FpCategory::Zero,
            (0, _) => FpCategory::Subnormal,
            (0x7C00u16, 0) => FpCategory::Infinite,
            (0x7C00u16, _) => FpCategory::Nan,
            _ => FpCategory::Normal,
        }
    }

    /// Returns a number that represents the sign of `self`.
    ///
    /// * `1.0` if the number is positive, `+0.0` or [`INFINITY`][f16::INFINITY]
    /// * `-1.0` if the number is negative, `-0.0` or [`NEG_INFINITY`][f16::NEG_INFINITY]
    /// * [`NAN`][f16::NAN] if the number is `NaN`
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    ///
    /// let f = f16::from_f32(3.5_f32);
    ///
    /// assert_eq!(f.signum(), f16::from_f32(1.0));
    /// assert_eq!(f16::NEG_INFINITY.signum(), f16::from_f32(-1.0));
    ///
    /// assert!(f16::NAN.signum().is_nan());
    /// ```
    #[must_use]
    pub const fn signum(self) -> f16 {
        if self.is_nan() {
            self
        } else if self.0 & 0x8000u16 != 0 {
            Self::NEG_ONE
        } else {
            Self::ONE
        }
    }

    /// Returns `true` if and only if `self` has a positive sign, including `+0.0`, `NaNs` with a
    /// positive sign bit and +∞.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    ///
    /// let nan = f16::NAN;
    /// let f = f16::from_f32(7.0_f32);
    /// let g = f16::from_f32(-7.0_f32);
    ///
    /// assert!(f.is_sign_positive());
    /// assert!(!g.is_sign_positive());
    /// // `NaN` can be either positive or negative
    /// assert!(nan.is_sign_positive() != nan.is_sign_negative());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_sign_positive(self) -> bool {
        self.0 & 0x8000u16 == 0
    }

    /// Returns `true` if and only if `self` has a negative sign, including `-0.0`, `NaNs` with a
    /// negative sign bit and −∞.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    ///
    /// let nan = f16::NAN;
    /// let f = f16::from_f32(7.0f32);
    /// let g = f16::from_f32(-7.0f32);
    ///
    /// assert!(!f.is_sign_negative());
    /// assert!(g.is_sign_negative());
    /// // `NaN` can be either positive or negative
    /// assert!(nan.is_sign_positive() != nan.is_sign_negative());
    /// ```
    #[inline]
    #[must_use]
    pub const fn is_sign_negative(self) -> bool {
        self.0 & 0x8000u16 != 0
    }

    /// Returns a number composed of the magnitude of `self` and the sign of `sign`.
    ///
    /// Equal to `self` if the sign of `self` and `sign` are the same, otherwise equal to `-self`.
    /// If `self` is NaN, then NaN with the sign of `sign` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use half::prelude::*;
    /// let f = f16::from_f32(3.5);
    ///
    /// assert_eq!(f.copysign(f16::from_f32(0.42)), f16::from_f32(3.5));
    /// assert_eq!(f.copysign(f16::from_f32(-0.42)), f16::from_f32(-3.5));
    /// assert_eq!((-f).copysign(f16::from_f32(0.42)), f16::from_f32(3.5));
    /// assert_eq!((-f).copysign(f16::from_f32(-0.42)), f16::from_f32(-3.5));
    ///
    /// assert!(f16::NAN.copysign(f16::from_f32(1.0)).is_nan());
    /// ```
    #[inline]
    #[must_use]
    pub const fn copysign(self, sign: f16) -> f16 {
        f16((sign.0 & 0x8000u16) | (self.0 & 0x7FFFu16))
    }

    /// Returns the maximum of the two numbers.
    ///
    /// If one of the arguments is NaN, then the other argument is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use half::prelude::*;
    /// let x = f16::from_f32(1.0);
    /// let y = f16::from_f32(2.0);
    ///
    /// assert_eq!(x.max(y), y);
    /// ```
    #[inline]
    #[must_use]
    pub fn max(self, other: f16) -> f16 {
        if other > self && !other.is_nan() {
            other
        } else {
            self
        }
    }

    /// Returns the minimum of the two numbers.
    ///
    /// If one of the arguments is NaN, then the other argument is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use half::prelude::*;
    /// let x = f16::from_f32(1.0);
    /// let y = f16::from_f32(2.0);
    ///
    /// assert_eq!(x.min(y), x);
    /// ```
    #[inline]
    #[must_use]
    pub fn min(self, other: f16) -> f16 {
        if other < self && !other.is_nan() {
            other
        } else {
            self
        }
    }

    /// Restrict a value to a certain interval unless it is NaN.
    ///
    /// Returns `max` if `self` is greater than `max`, and `min` if `self` is less than `min`.
    /// Otherwise this returns `self`.
    ///
    /// Note that this function returns NaN if the initial value was NaN as well.
    ///
    /// # Panics
    /// Panics if `min > max`, `min` is NaN, or `max` is NaN.
    ///
    /// # Examples
    ///
    /// ```
    /// # use half::prelude::*;
    /// assert!(f16::from_f32(-3.0).clamp(f16::from_f32(-2.0), f16::from_f32(1.0)) == f16::from_f32(-2.0));
    /// assert!(f16::from_f32(0.0).clamp(f16::from_f32(-2.0), f16::from_f32(1.0)) == f16::from_f32(0.0));
    /// assert!(f16::from_f32(2.0).clamp(f16::from_f32(-2.0), f16::from_f32(1.0)) == f16::from_f32(1.0));
    /// assert!(f16::NAN.clamp(f16::from_f32(-2.0), f16::from_f32(1.0)).is_nan());
    /// ```
    #[inline]
    #[must_use]
    pub fn clamp(self, min: f16, max: f16) -> f16 {
        assert!(min <= max);
        let mut x = self;
        if x < min {
            x = min;
        }
        if x > max {
            x = max;
        }
        x
    }

    /// Returns the ordering between `self` and `other`.
    ///
    /// Unlike the standard partial comparison between floating point numbers,
    /// this comparison always produces an ordering in accordance to
    /// the `totalOrder` predicate as defined in the IEEE 754 (2008 revision)
    /// floating point standard. The values are ordered in the following sequence:
    ///
    /// - negative quiet NaN
    /// - negative signaling NaN
    /// - negative infinity
    /// - negative numbers
    /// - negative subnormal numbers
    /// - negative zero
    /// - positive zero
    /// - positive subnormal numbers
    /// - positive numbers
    /// - positive infinity
    /// - positive signaling NaN
    /// - positive quiet NaN.
    ///
    /// The ordering established by this function does not always agree with the
    /// [`PartialOrd`] and [`PartialEq`] implementations of `f16`. For example,
    /// they consider negative and positive zero equal, while `total_cmp`
    /// doesn't.
    ///
    /// The interpretation of the signaling NaN bit follows the definition in
    /// the IEEE 754 standard, which may not match the interpretation by some of
    /// the older, non-conformant (e.g. MIPS) hardware implementations.
    ///
    /// # Examples
    /// ```
    /// # use half::f16;
    /// let mut v: Vec<f16> = vec![];
    /// v.push(f16::ONE);
    /// v.push(f16::INFINITY);
    /// v.push(f16::NEG_INFINITY);
    /// v.push(f16::NAN);
    /// v.push(f16::MAX_SUBNORMAL);
    /// v.push(-f16::MAX_SUBNORMAL);
    /// v.push(f16::ZERO);
    /// v.push(f16::NEG_ZERO);
    /// v.push(f16::NEG_ONE);
    /// v.push(f16::MIN_POSITIVE);
    ///
    /// v.sort_by(|a, b| a.total_cmp(&b));
    ///
    /// assert!(v
    ///     .into_iter()
    ///     .zip(
    ///         [
    ///             f16::NEG_INFINITY,
    ///             f16::NEG_ONE,
    ///             -f16::MAX_SUBNORMAL,
    ///             f16::NEG_ZERO,
    ///             f16::ZERO,
    ///             f16::MAX_SUBNORMAL,
    ///             f16::MIN_POSITIVE,
    ///             f16::ONE,
    ///             f16::INFINITY,
    ///             f16::NAN
    ///         ]
    ///         .iter()
    ///     )
    ///     .all(|(a, b)| a.to_bits() == b.to_bits()));
    /// ```
    // Implementation based on: https://doc.rust-lang.org/std/primitive.f32.html#method.total_cmp
    #[inline]
    #[must_use]
    pub fn total_cmp(&self, other: &Self) -> Ordering {
        let mut left = self.to_bits() as i16;
        let mut right = other.to_bits() as i16;
        left ^= (((left >> 15) as u16) >> 1) as i16;
        right ^= (((right >> 15) as u16) >> 1) as i16;
        left.cmp(&right)
    }

    /// Alternate serialize adapter for serializing as a float.
    ///
    /// By default, [`struct@f16`] serializes as a newtype of [`u16`]. This is an alternate serialize
    /// implementation that serializes as an [`f32`] value. It is designed for use with
    /// `serialize_with` serde attributes. Deserialization from `f32` values is already supported by
    /// the default deserialize implementation.
    ///
    /// # Examples
    ///
    /// A demonstration on how to use this adapater:
    ///
    /// ```
    /// use serde::{Serialize, Deserialize};
    /// use half::f16;
    ///
    /// #[derive(Serialize, Deserialize)]
    /// struct MyStruct {
    ///     #[serde(serialize_with = "f16::serialize_as_f32")]
    ///     value: f16 // Will be serialized as f32 instead of u16
    /// }
    /// ```
    #[cfg(feature = "serde")]
    pub fn serialize_as_f32<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f32(self.to_f32())
    }

    /// Alternate serialize adapter for serializing as a string.
    ///
    /// By default, [`struct@f16`] serializes as a newtype of [`u16`]. This is an alternate serialize
    /// implementation that serializes as a string value. It is designed for use with
    /// `serialize_with` serde attributes. Deserialization from string values is already supported
    /// by the default deserialize implementation.
    ///
    /// # Examples
    ///
    /// A demonstration on how to use this adapater:
    ///
    /// ```
    /// use serde::{Serialize, Deserialize};
    /// use half::f16;
    ///
    /// #[derive(Serialize, Deserialize)]
    /// struct MyStruct {
    ///     #[serde(serialize_with = "f16::serialize_as_string")]
    ///     value: f16 // Will be serialized as a string instead of u16
    /// }
    /// ```
    #[cfg(all(feature = "serde", feature = "alloc"))]
    pub fn serialize_as_string<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }

    /// Approximate number of [`struct@f16`] significant digits in base 10
    pub const DIGITS: u32 = 3;
    /// [`struct@f16`]
    /// [machine epsilon](https://en.wikipedia.org/wiki/Machine_epsilon) value
    ///
    /// This is the difference between 1.0 and the next largest representable number.
    pub const EPSILON: f16 = f16(0x1400u16);
    /// [`struct@f16`] positive Infinity (+∞)
    pub const INFINITY: f16 = f16(0x7C00u16);
    /// Number of [`struct@f16`] significant digits in base 2
    pub const MANTISSA_DIGITS: u32 = 11;
    /// Largest finite [`struct@f16`] value
    pub const MAX: f16 = f16(0x7BFF);
    /// Maximum possible [`struct@f16`] power of 10 exponent
    pub const MAX_10_EXP: i32 = 4;
    /// Maximum possible [`struct@f16`] power of 2 exponent
    pub const MAX_EXP: i32 = 16;
    /// Smallest finite [`struct@f16`] value
    pub const MIN: f16 = f16(0xFBFF);
    /// Minimum possible normal [`struct@f16`] power of 10 exponent
    pub const MIN_10_EXP: i32 = -4;
    /// One greater than the minimum possible normal [`struct@f16`] power of 2 exponent
    pub const MIN_EXP: i32 = -13;
    /// Smallest positive normal [`struct@f16`] value
    pub const MIN_POSITIVE: f16 = f16(0x0400u16);
    /// [`struct@f16`] Not a Number (NaN)
    pub const NAN: f16 = f16(0x7E00u16);
    /// [`struct@f16`] negative infinity (-∞)
    pub const NEG_INFINITY: f16 = f16(0xFC00u16);
    /// The radix or base of the internal representation of [`struct@f16`]
    pub const RADIX: u32 = 2;

    /// Minimum positive subnormal [`struct@f16`] value
    pub const MIN_POSITIVE_SUBNORMAL: f16 = f16(0x0001u16);
    /// Maximum subnormal [`struct@f16`] value
    pub const MAX_SUBNORMAL: f16 = f16(0x03FFu16);

    /// [`struct@f16`] 1
    pub const ONE: f16 = f16(0x3C00u16);
    /// [`struct@f16`] 0
    pub const ZERO: f16 = f16(0x0000u16);
    /// [`struct@f16`] -0
    pub const NEG_ZERO: f16 = f16(0x8000u16);
    /// [`struct@f16`] -1
    pub const NEG_ONE: f16 = f16(0xBC00u16);

    /// [`struct@f16`] Euler's number (ℯ)
    pub const E: f16 = f16(0x4170u16);
    /// [`struct@f16`] Archimedes' constant (π)
    pub const PI: f16 = f16(0x4248u16);
    /// [`struct@f16`] 1/π
    pub const FRAC_1_PI: f16 = f16(0x3518u16);
    /// [`struct@f16`] 1/√2
    pub const FRAC_1_SQRT_2: f16 = f16(0x39A8u16);
    /// [`struct@f16`] 2/π
    pub const FRAC_2_PI: f16 = f16(0x3918u16);
    /// [`struct@f16`] 2/√π
    pub const FRAC_2_SQRT_PI: f16 = f16(0x3C83u16);
    /// [`struct@f16`] π/2
    pub const FRAC_PI_2: f16 = f16(0x3E48u16);
    /// [`struct@f16`] π/3
    pub const FRAC_PI_3: f16 = f16(0x3C30u16);
    /// [`struct@f16`] π/4
    pub const FRAC_PI_4: f16 = f16(0x3A48u16);
    /// [`struct@f16`] π/6
    pub const FRAC_PI_6: f16 = f16(0x3830u16);
    /// [`struct@f16`] π/8
    pub const FRAC_PI_8: f16 = f16(0x3648u16);
    /// [`struct@f16`] 𝗅𝗇 10
    pub const LN_10: f16 = f16(0x409Bu16);
    /// [`struct@f16`] 𝗅𝗇 2
    pub const LN_2: f16 = f16(0x398Cu16);
    /// [`struct@f16`] 𝗅𝗈𝗀₁₀ℯ
    pub const LOG10_E: f16 = f16(0x36F3u16);
    /// [`struct@f16`] 𝗅𝗈𝗀₁₀2
    pub const LOG10_2: f16 = f16(0x34D1u16);
    /// [`struct@f16`] 𝗅𝗈𝗀₂ℯ
    pub const LOG2_E: f16 = f16(0x3DC5u16);
    /// [`struct@f16`] 𝗅𝗈𝗀₂10
    pub const LOG2_10: f16 = f16(0x42A5u16);
    /// [`struct@f16`] √2
    pub const SQRT_2: f16 = f16(0x3DA8u16);
}

impl From<f16> for f32 {
    #[inline]
    fn from(x: f16) -> f32 {
        x.to_f32()
    }
}

impl From<f16> for f64 {
    #[inline]
    fn from(x: f16) -> f64 {
        x.to_f64()
    }
}

impl From<i8> for f16 {
    #[inline]
    fn from(x: i8) -> f16 {
        // Convert to f32, then to f16
        f16::from_f32(f32::from(x))
    }
}

impl From<u8> for f16 {
    #[inline]
    fn from(x: u8) -> f16 {
        // Convert to f32, then to f16
        f16::from_f32(f32::from(x))
    }
}

impl PartialEq for f16 {
    fn eq(&self, other: &f16) -> bool {
        if self.is_nan() || other.is_nan() {
            false
        } else {
            (self.0 == other.0) || ((self.0 | other.0) & 0x7FFFu16 == 0)
        }
    }
}

impl PartialOrd for f16 {
    fn partial_cmp(&self, other: &f16) -> Option<Ordering> {
        if self.is_nan() || other.is_nan() {
            None
        } else {
            let neg = self.0 & 0x8000u16 != 0;
            let other_neg = other.0 & 0x8000u16 != 0;
            match (neg, other_neg) {
                (false, false) => Some(self.0.cmp(&other.0)),
                (false, true) => {
                    if (self.0 | other.0) & 0x7FFFu16 == 0 {
                        Some(Ordering::Equal)
                    } else {
                        Some(Ordering::Greater)
                    }
                }
                (true, false) => {
                    if (self.0 | other.0) & 0x7FFFu16 == 0 {
                        Some(Ordering::Equal)
                    } else {
                        Some(Ordering::Less)
                    }
                }
                (true, true) => Some(other.0.cmp(&self.0)),
            }
        }
    }

    fn lt(&self, other: &f16) -> bool {
        if self.is_nan() || other.is_nan() {
            false
        } else {
            let neg = self.0 & 0x8000u16 != 0;
            let other_neg = other.0 & 0x8000u16 != 0;
            match (neg, other_neg) {
                (false, false) => self.0 < other.0,
                (false, true) => false,
                (true, false) => (self.0 | other.0) & 0x7FFFu16 != 0,
                (true, true) => self.0 > other.0,
            }
        }
    }

    fn le(&self, other: &f16) -> bool {
        if self.is_nan() || other.is_nan() {
            false
        } else {
            let neg = self.0 & 0x8000u16 != 0;
            let other_neg = other.0 & 0x8000u16 != 0;
            match (neg, other_neg) {
                (false, false) => self.0 <= other.0,
                (false, true) => (self.0 | other.0) & 0x7FFFu16 == 0,
                (true, false) => true,
                (true, true) => self.0 >= other.0,
            }
        }
    }

    fn gt(&self, other: &f16) -> bool {
        if self.is_nan() || other.is_nan() {
            false
        } else {
            let neg = self.0 & 0x8000u16 != 0;
            let other_neg = other.0 & 0x8000u16 != 0;
            match (neg, other_neg) {
                (false, false) => self.0 > other.0,
                (false, true) => (self.0 | other.0) & 0x7FFFu16 != 0,
                (true, false) => false,
                (true, true) => self.0 < other.0,
            }
        }
    }

    fn ge(&self, other: &f16) -> bool {
        if self.is_nan() || other.is_nan() {
            false
        } else {
            let neg = self.0 & 0x8000u16 != 0;
            let other_neg = other.0 & 0x8000u16 != 0;
            match (neg, other_neg) {
                (false, false) => self.0 >= other.0,
                (false, true) => true,
                (true, false) => (self.0 | other.0) & 0x7FFFu16 == 0,
                (true, true) => self.0 <= other.0,
            }
        }
    }
}

#[cfg(not(target_arch = "spirv"))]
impl FromStr for f16 {
    type Err = ParseFloatError;
    fn from_str(src: &str) -> Result<f16, ParseFloatError> {
        f32::from_str(src).map(f16::from_f32)
    }
}

#[cfg(not(target_arch = "spirv"))]
impl Debug for f16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        Debug::fmt(&self.to_f32(), f)
    }
}

#[cfg(not(target_arch = "spirv"))]
impl Display for f16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        Display::fmt(&self.to_f32(), f)
    }
}

#[cfg(not(target_arch = "spirv"))]
impl LowerExp for f16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{:e}", self.to_f32())
    }
}

#[cfg(not(target_arch = "spirv"))]
impl UpperExp for f16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{:E}", self.to_f32())
    }
}

#[cfg(not(target_arch = "spirv"))]
impl Binary for f16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{:b}", self.0)
    }
}

#[cfg(not(target_arch = "spirv"))]
impl Octal for f16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{:o}", self.0)
    }
}

#[cfg(not(target_arch = "spirv"))]
impl LowerHex for f16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{:x}", self.0)
    }
}

#[cfg(not(target_arch = "spirv"))]
impl UpperHex for f16 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        write!(f, "{:X}", self.0)
    }
}

impl Neg for f16 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self(self.0 ^ 0x8000)
    }
}

impl Neg for &f16 {
    type Output = <f16 as Neg>::Output;

    #[inline]
    fn neg(self) -> Self::Output {
        Neg::neg(*self)
    }
}

impl Add for f16 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        f16(arch::add_f16(self.0, rhs.0))
    }
}

impl Add<&f16> for f16 {
    type Output = <f16 as Add<f16>>::Output;

    #[inline]
    fn add(self, rhs: &f16) -> Self::Output {
        self.add(*rhs)
    }
}

impl Add<&f16> for &f16 {
    type Output = <f16 as Add<f16>>::Output;

    #[inline]
    fn add(self, rhs: &f16) -> Self::Output {
        (*self).add(*rhs)
    }
}

impl Add<f16> for &f16 {
    type Output = <f16 as Add<f16>>::Output;

    #[inline]
    fn add(self, rhs: f16) -> Self::Output {
        (*self).add(rhs)
    }
}

impl AddAssign for f16 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = (*self).add(rhs);
    }
}

impl AddAssign<&f16> for f16 {
    #[inline]
    fn add_assign(&mut self, rhs: &f16) {
        *self = (*self).add(rhs);
    }
}

impl Sub for f16 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        f16(arch::subtract_f16(self.0, rhs.0))
    }
}

impl Sub<&f16> for f16 {
    type Output = <f16 as Sub<f16>>::Output;

    #[inline]
    fn sub(self, rhs: &f16) -> Self::Output {
        self.sub(*rhs)
    }
}

impl Sub<&f16> for &f16 {
    type Output = <f16 as Sub<f16>>::Output;

    #[inline]
    fn sub(self, rhs: &f16) -> Self::Output {
        (*self).sub(*rhs)
    }
}

impl Sub<f16> for &f16 {
    type Output = <f16 as Sub<f16>>::Output;

    #[inline]
    fn sub(self, rhs: f16) -> Self::Output {
        (*self).sub(rhs)
    }
}

impl SubAssign for f16 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = (*self).sub(rhs);
    }
}

impl SubAssign<&f16> for f16 {
    #[inline]
    fn sub_assign(&mut self, rhs: &f16) {
        *self = (*self).sub(rhs);
    }
}

impl Mul for f16 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        f16(arch::multiply_f16(self.0, rhs.0))
    }
}

impl Mul<&f16> for f16 {
    type Output = <f16 as Mul<f16>>::Output;

    #[inline]
    fn mul(self, rhs: &f16) -> Self::Output {
        self.mul(*rhs)
    }
}

impl Mul<&f16> for &f16 {
    type Output = <f16 as Mul<f16>>::Output;

    #[inline]
    fn mul(self, rhs: &f16) -> Self::Output {
        (*self).mul(*rhs)
    }
}

impl Mul<f16> for &f16 {
    type Output = <f16 as Mul<f16>>::Output;

    #[inline]
    fn mul(self, rhs: f16) -> Self::Output {
        (*self).mul(rhs)
    }
}

impl MulAssign for f16 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = (*self).mul(rhs);
    }
}

impl MulAssign<&f16> for f16 {
    #[inline]
    fn mul_assign(&mut self, rhs: &f16) {
        *self = (*self).mul(rhs);
    }
}

impl Div for f16 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        f16(arch::divide_f16(self.0, rhs.0))
    }
}

impl Div<&f16> for f16 {
    type Output = <f16 as Div<f16>>::Output;

    #[inline]
    fn div(self, rhs: &f16) -> Self::Output {
        self.div(*rhs)
    }
}

impl Div<&f16> for &f16 {
    type Output = <f16 as Div<f16>>::Output;

    #[inline]
    fn div(self, rhs: &f16) -> Self::Output {
        (*self).div(*rhs)
    }
}

impl Div<f16> for &f16 {
    type Output = <f16 as Div<f16>>::Output;

    #[inline]
    fn div(self, rhs: f16) -> Self::Output {
        (*self).div(rhs)
    }
}

impl DivAssign for f16 {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        *self = (*self).div(rhs);
    }
}

impl DivAssign<&f16> for f16 {
    #[inline]
    fn div_assign(&mut self, rhs: &f16) {
        *self = (*self).div(rhs);
    }
}

impl Rem for f16 {
    type Output = Self;

    #[inline]
    fn rem(self, rhs: Self) -> Self::Output {
        f16(arch::remainder_f16(self.0, rhs.0))
    }
}

impl Rem<&f16> for f16 {
    type Output = <f16 as Rem<f16>>::Output;

    #[inline]
    fn rem(self, rhs: &f16) -> Self::Output {
        self.rem(*rhs)
    }
}

impl Rem<&f16> for &f16 {
    type Output = <f16 as Rem<f16>>::Output;

    #[inline]
    fn rem(self, rhs: &f16) -> Self::Output {
        (*self).rem(*rhs)
    }
}

impl Rem<f16> for &f16 {
    type Output = <f16 as Rem<f16>>::Output;

    #[inline]
    fn rem(self, rhs: f16) -> Self::Output {
        (*self).rem(rhs)
    }
}

impl RemAssign for f16 {
    #[inline]
    fn rem_assign(&mut self, rhs: Self) {
        *self = (*self).rem(rhs);
    }
}

impl RemAssign<&f16> for f16 {
    #[inline]
    fn rem_assign(&mut self, rhs: &f16) {
        *self = (*self).rem(rhs);
    }
}

impl Product for f16 {
    #[inline]
    fn product<I: Iterator<Item = Self>>(iter: I) -> Self {
        f16(arch::product_f16(iter.map(|f| f.to_bits())))
    }
}

impl<'a> Product<&'a f16> for f16 {
    #[inline]
    fn product<I: Iterator<Item = &'a f16>>(iter: I) -> Self {
        f16(arch::product_f16(iter.map(|f| f.to_bits())))
    }
}

impl Sum for f16 {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        f16(arch::sum_f16(iter.map(|f| f.to_bits())))
    }
}

impl<'a> Sum<&'a f16> for f16 {
    #[inline]
    fn sum<I: Iterator<Item = &'a f16>>(iter: I) -> Self {
        f16(arch::sum_f16(iter.map(|f| f.to_bits())))
    }
}

#[cfg(feature = "serde")]
struct Visitor;

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for f16 {
    fn deserialize<D>(deserializer: D) -> Result<f16, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer.deserialize_newtype_struct("f16", Visitor)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::de::Visitor<'de> for Visitor {
    type Value = f16;

    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(formatter, "tuple struct f16")
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(f16(<u16 as Deserialize>::deserialize(deserializer)?))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.parse().map_err(|_| {
            serde::de::Error::invalid_value(serde::de::Unexpected::Str(v), &"a float string")
        })
    }

    fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(f16::from_f32(v))
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(f16::from_f64(v))
    }
}

#[allow(
    clippy::cognitive_complexity,
    clippy::float_cmp,
    clippy::neg_cmp_op_on_partial_ord
)]
#[cfg(test)]
mod test {
    use super::*;
    #[allow(unused_imports)]
    use core::cmp::Ordering;
    #[cfg(feature = "num-traits")]
    use num_traits::{AsPrimitive, FromBytes, FromPrimitive, ToBytes, ToPrimitive};
    use quickcheck_macros::quickcheck;

    #[cfg(feature = "num-traits")]
    #[test]
    fn as_primitive() {
        let two = f16::from_f32(2.0);
        assert_eq!(<i32 as AsPrimitive<f16>>::as_(2), two);
        assert_eq!(<f16 as AsPrimitive<i32>>::as_(two), 2);

        assert_eq!(<f32 as AsPrimitive<f16>>::as_(2.0), two);
        assert_eq!(<f16 as AsPrimitive<f32>>::as_(two), 2.0);

        assert_eq!(<f64 as AsPrimitive<f16>>::as_(2.0), two);
        assert_eq!(<f16 as AsPrimitive<f64>>::as_(two), 2.0);
    }

    #[cfg(feature = "num-traits")]
    #[test]
    fn to_primitive() {
        let two = f16::from_f32(2.0);
        assert_eq!(ToPrimitive::to_i32(&two).unwrap(), 2i32);
        assert_eq!(ToPrimitive::to_f32(&two).unwrap(), 2.0f32);
        assert_eq!(ToPrimitive::to_f64(&two).unwrap(), 2.0f64);
    }

    #[cfg(feature = "num-traits")]
    #[test]
    fn from_primitive() {
        let two = f16::from_f32(2.0);
        assert_eq!(<f16 as FromPrimitive>::from_i32(2).unwrap(), two);
        assert_eq!(<f16 as FromPrimitive>::from_f32(2.0).unwrap(), two);
        assert_eq!(<f16 as FromPrimitive>::from_f64(2.0).unwrap(), two);
    }

    #[cfg(feature = "num-traits")]
    #[test]
    fn to_and_from_bytes() {
        let two = f16::from_f32(2.0);
        assert_eq!(<f16 as ToBytes>::to_le_bytes(&two), [0, 64]);
        assert_eq!(<f16 as FromBytes>::from_le_bytes(&[0, 64]), two);
        assert_eq!(<f16 as ToBytes>::to_be_bytes(&two), [64, 0]);
        assert_eq!(<f16 as FromBytes>::from_be_bytes(&[64, 0]), two);
    }

    #[test]
    fn test_f16_consts() {
        // DIGITS
        let digits = ((f16::MANTISSA_DIGITS as f32 - 1.0) * 2f32.log10()).floor() as u32;
        assert_eq!(f16::DIGITS, digits);
        // sanity check to show test is good
        let digits32 = ((core::f32::MANTISSA_DIGITS as f32 - 1.0) * 2f32.log10()).floor() as u32;
        assert_eq!(core::f32::DIGITS, digits32);

        // EPSILON
        let one = f16::from_f32(1.0);
        let one_plus_epsilon = f16::from_bits(one.to_bits() + 1);
        let epsilon = f16::from_f32(one_plus_epsilon.to_f32() - 1.0);
        assert_eq!(f16::EPSILON, epsilon);
        // sanity check to show test is good
        let one_plus_epsilon32 = f32::from_bits(1.0f32.to_bits() + 1);
        let epsilon32 = one_plus_epsilon32 - 1f32;
        assert_eq!(core::f32::EPSILON, epsilon32);

        // MAX, MIN and MIN_POSITIVE
        let max = f16::from_bits(f16::INFINITY.to_bits() - 1);
        let min = f16::from_bits(f16::NEG_INFINITY.to_bits() - 1);
        let min_pos = f16::from_f32(2f32.powi(f16::MIN_EXP - 1));
        assert_eq!(f16::MAX, max);
        assert_eq!(f16::MIN, min);
        assert_eq!(f16::MIN_POSITIVE, min_pos);
        // sanity check to show test is good
        let max32 = f32::from_bits(core::f32::INFINITY.to_bits() - 1);
        let min32 = f32::from_bits(core::f32::NEG_INFINITY.to_bits() - 1);
        let min_pos32 = 2f32.powi(core::f32::MIN_EXP - 1);
        assert_eq!(core::f32::MAX, max32);
        assert_eq!(core::f32::MIN, min32);
        assert_eq!(core::f32::MIN_POSITIVE, min_pos32);

        // MIN_10_EXP and MAX_10_EXP
        let ten_to_min = 10f32.powi(f16::MIN_10_EXP);
        assert!(ten_to_min / 10.0 < f16::MIN_POSITIVE.to_f32());
        assert!(ten_to_min > f16::MIN_POSITIVE.to_f32());
        let ten_to_max = 10f32.powi(f16::MAX_10_EXP);
        assert!(ten_to_max < f16::MAX.to_f32());
        assert!(ten_to_max * 10.0 > f16::MAX.to_f32());
        // sanity check to show test is good
        let ten_to_min32 = 10f64.powi(core::f32::MIN_10_EXP);
        assert!(ten_to_min32 / 10.0 < f64::from(core::f32::MIN_POSITIVE));
        assert!(ten_to_min32 > f64::from(core::f32::MIN_POSITIVE));
        let ten_to_max32 = 10f64.powi(core::f32::MAX_10_EXP);
        assert!(ten_to_max32 < f64::from(core::f32::MAX));
        assert!(ten_to_max32 * 10.0 > f64::from(core::f32::MAX));
    }

    #[test]
    fn test_f16_consts_from_f32() {
        let one = f16::from_f32(1.0);
        let zero = f16::from_f32(0.0);
        let neg_zero = f16::from_f32(-0.0);
        let neg_one = f16::from_f32(-1.0);
        let inf = f16::from_f32(core::f32::INFINITY);
        let neg_inf = f16::from_f32(core::f32::NEG_INFINITY);
        let nan = f16::from_f32(core::f32::NAN);

        assert_eq!(f16::ONE, one);
        assert_eq!(f16::ZERO, zero);
        assert!(zero.is_sign_positive());
        assert_eq!(f16::NEG_ZERO, neg_zero);
        assert!(neg_zero.is_sign_negative());
        assert_eq!(f16::NEG_ONE, neg_one);
        assert!(neg_one.is_sign_negative());
        assert_eq!(f16::INFINITY, inf);
        assert_eq!(f16::NEG_INFINITY, neg_inf);
        assert!(nan.is_nan());
        assert!(f16::NAN.is_nan());

        let e = f16::from_f32(core::f32::consts::E);
        let pi = f16::from_f32(core::f32::consts::PI);
        let frac_1_pi = f16::from_f32(core::f32::consts::FRAC_1_PI);
        let frac_1_sqrt_2 = f16::from_f32(core::f32::consts::FRAC_1_SQRT_2);
        let frac_2_pi = f16::from_f32(core::f32::consts::FRAC_2_PI);
        let frac_2_sqrt_pi = f16::from_f32(core::f32::consts::FRAC_2_SQRT_PI);
        let frac_pi_2 = f16::from_f32(core::f32::consts::FRAC_PI_2);
        let frac_pi_3 = f16::from_f32(core::f32::consts::FRAC_PI_3);
        let frac_pi_4 = f16::from_f32(core::f32::consts::FRAC_PI_4);
        let frac_pi_6 = f16::from_f32(core::f32::consts::FRAC_PI_6);
        let frac_pi_8 = f16::from_f32(core::f32::consts::FRAC_PI_8);
        let ln_10 = f16::from_f32(core::f32::consts::LN_10);
        let ln_2 = f16::from_f32(core::f32::consts::LN_2);
        let log10_e = f16::from_f32(core::f32::consts::LOG10_E);
        // core::f32::consts::LOG10_2 requires rustc 1.43.0
        let log10_2 = f16::from_f32(2f32.log10());
        let log2_e = f16::from_f32(core::f32::consts::LOG2_E);
        // core::f32::consts::LOG2_10 requires rustc 1.43.0
        let log2_10 = f16::from_f32(10f32.log2());
        let sqrt_2 = f16::from_f32(core::f32::consts::SQRT_2);

        assert_eq!(f16::E, e);
        assert_eq!(f16::PI, pi);
        assert_eq!(f16::FRAC_1_PI, frac_1_pi);
        assert_eq!(f16::FRAC_1_SQRT_2, frac_1_sqrt_2);
        assert_eq!(f16::FRAC_2_PI, frac_2_pi);
        assert_eq!(f16::FRAC_2_SQRT_PI, frac_2_sqrt_pi);
        assert_eq!(f16::FRAC_PI_2, frac_pi_2);
        assert_eq!(f16::FRAC_PI_3, frac_pi_3);
        assert_eq!(f16::FRAC_PI_4, frac_pi_4);
        assert_eq!(f16::FRAC_PI_6, frac_pi_6);
        assert_eq!(f16::FRAC_PI_8, frac_pi_8);
        assert_eq!(f16::LN_10, ln_10);
        assert_eq!(f16::LN_2, ln_2);
        assert_eq!(f16::LOG10_E, log10_e);
        assert_eq!(f16::LOG10_2, log10_2);
        assert_eq!(f16::LOG2_E, log2_e);
        assert_eq!(f16::LOG2_10, log2_10);
        assert_eq!(f16::SQRT_2, sqrt_2);
    }

    #[test]
    fn test_f16_consts_from_f64() {
        let one = f16::from_f64(1.0);
        let zero = f16::from_f64(0.0);
        let neg_zero = f16::from_f64(-0.0);
        let inf = f16::from_f64(core::f64::INFINITY);
        let neg_inf = f16::from_f64(core::f64::NEG_INFINITY);
        let nan = f16::from_f64(core::f64::NAN);

        assert_eq!(f16::ONE, one);
        assert_eq!(f16::ZERO, zero);
        assert!(zero.is_sign_positive());
        assert_eq!(f16::NEG_ZERO, neg_zero);
        assert!(neg_zero.is_sign_negative());
        assert_eq!(f16::INFINITY, inf);
        assert_eq!(f16::NEG_INFINITY, neg_inf);
        assert!(nan.is_nan());
        assert!(f16::NAN.is_nan());

        let e = f16::from_f64(core::f64::consts::E);
        let pi = f16::from_f64(core::f64::consts::PI);
        let frac_1_pi = f16::from_f64(core::f64::consts::FRAC_1_PI);
        let frac_1_sqrt_2 = f16::from_f64(core::f64::consts::FRAC_1_SQRT_2);
        let frac_2_pi = f16::from_f64(core::f64::consts::FRAC_2_PI);
        let frac_2_sqrt_pi = f16::from_f64(core::f64::consts::FRAC_2_SQRT_PI);
        let frac_pi_2 = f16::from_f64(core::f64::consts::FRAC_PI_2);
        let frac_pi_3 = f16::from_f64(core::f64::consts::FRAC_PI_3);
        let frac_pi_4 = f16::from_f64(core::f64::consts::FRAC_PI_4);
        let frac_pi_6 = f16::from_f64(core::f64::consts::FRAC_PI_6);
        let frac_pi_8 = f16::from_f64(core::f64::consts::FRAC_PI_8);
        let ln_10 = f16::from_f64(core::f64::consts::LN_10);
        let ln_2 = f16::from_f64(core::f64::consts::LN_2);
        let log10_e = f16::from_f64(core::f64::consts::LOG10_E);
        // core::f64::consts::LOG10_2 requires rustc 1.43.0
        let log10_2 = f16::from_f64(2f64.log10());
        let log2_e = f16::from_f64(core::f64::consts::LOG2_E);
        // core::f64::consts::LOG2_10 requires rustc 1.43.0
        let log2_10 = f16::from_f64(10f64.log2());
        let sqrt_2 = f16::from_f64(core::f64::consts::SQRT_2);

        assert_eq!(f16::E, e);
        assert_eq!(f16::PI, pi);
        assert_eq!(f16::FRAC_1_PI, frac_1_pi);
        assert_eq!(f16::FRAC_1_SQRT_2, frac_1_sqrt_2);
        assert_eq!(f16::FRAC_2_PI, frac_2_pi);
        assert_eq!(f16::FRAC_2_SQRT_PI, frac_2_sqrt_pi);
        assert_eq!(f16::FRAC_PI_2, frac_pi_2);
        assert_eq!(f16::FRAC_PI_3, frac_pi_3);
        assert_eq!(f16::FRAC_PI_4, frac_pi_4);
        assert_eq!(f16::FRAC_PI_6, frac_pi_6);
        assert_eq!(f16::FRAC_PI_8, frac_pi_8);
        assert_eq!(f16::LN_10, ln_10);
        assert_eq!(f16::LN_2, ln_2);
        assert_eq!(f16::LOG10_E, log10_e);
        assert_eq!(f16::LOG10_2, log10_2);
        assert_eq!(f16::LOG2_E, log2_e);
        assert_eq!(f16::LOG2_10, log2_10);
        assert_eq!(f16::SQRT_2, sqrt_2);
    }

    #[test]
    fn test_nan_conversion_to_smaller() {
        let nan64 = f64::from_bits(0x7FF0_0000_0000_0001u64);
        let neg_nan64 = f64::from_bits(0xFFF0_0000_0000_0001u64);
        let nan32 = f32::from_bits(0x7F80_0001u32);
        let neg_nan32 = f32::from_bits(0xFF80_0001u32);
        let nan32_from_64 = nan64 as f32;
        let neg_nan32_from_64 = neg_nan64 as f32;
        let nan16_from_64 = f16::from_f64(nan64);
        let neg_nan16_from_64 = f16::from_f64(neg_nan64);
        let nan16_from_32 = f16::from_f32(nan32);
        let neg_nan16_from_32 = f16::from_f32(neg_nan32);

        assert!(nan64.is_nan() && nan64.is_sign_positive());
        assert!(neg_nan64.is_nan() && neg_nan64.is_sign_negative());
        assert!(nan32.is_nan() && nan32.is_sign_positive());
        assert!(neg_nan32.is_nan() && neg_nan32.is_sign_negative());

        // f32/f64 NaN conversion sign is non-deterministic: https://github.com/starkat99/half-rs/issues/103
        assert!(nan32_from_64.is_nan());
        assert!(neg_nan32_from_64.is_nan());
        assert!(nan16_from_64.is_nan());
        assert!(neg_nan16_from_64.is_nan());
        assert!(nan16_from_32.is_nan());
        assert!(neg_nan16_from_32.is_nan());
    }

    #[test]
    fn test_nan_conversion_to_larger() {
        let nan16 = f16::from_bits(0x7C01u16);
        let neg_nan16 = f16::from_bits(0xFC01u16);
        let nan32 = f32::from_bits(0x7F80_0001u32);
        let neg_nan32 = f32::from_bits(0xFF80_0001u32);
        let nan32_from_16 = f32::from(nan16);
        let neg_nan32_from_16 = f32::from(neg_nan16);
        let nan64_from_16 = f64::from(nan16);
        let neg_nan64_from_16 = f64::from(neg_nan16);
        let nan64_from_32 = f64::from(nan32);
        let neg_nan64_from_32 = f64::from(neg_nan32);

        assert!(nan16.is_nan() && nan16.is_sign_positive());
        assert!(neg_nan16.is_nan() && neg_nan16.is_sign_negative());
        assert!(nan32.is_nan() && nan32.is_sign_positive());
        assert!(neg_nan32.is_nan() && neg_nan32.is_sign_negative());

        // f32/f64 NaN conversion sign is non-deterministic: https://github.com/starkat99/half-rs/issues/103
        assert!(nan32_from_16.is_nan());
        assert!(neg_nan32_from_16.is_nan());
        assert!(nan64_from_16.is_nan());
        assert!(neg_nan64_from_16.is_nan());
        assert!(nan64_from_32.is_nan());
        assert!(neg_nan64_from_32.is_nan());
    }

    #[test]
    fn test_f16_to_f32() {
        let f = f16::from_f32(7.0);
        assert_eq!(f.to_f32(), 7.0f32);

        // 7.1 is NOT exactly representable in 16-bit, it's rounded
        let f = f16::from_f32(7.1);
        let diff = (f.to_f32() - 7.1f32).abs();
        // diff must be <= 4 * EPSILON, as 7 has two more significant bits than 1
        assert!(diff <= 4.0 * f16::EPSILON.to_f32());

        assert_eq!(f16::from_bits(0x0000_0001).to_f32(), 2.0f32.powi(-24));
        assert_eq!(f16::from_bits(0x0000_0005).to_f32(), 5.0 * 2.0f32.powi(-24));

        assert_eq!(f16::from_bits(0x0000_0001), f16::from_f32(2.0f32.powi(-24)));
        assert_eq!(
            f16::from_bits(0x0000_0005),
            f16::from_f32(5.0 * 2.0f32.powi(-24))
        );
    }

    #[test]
    fn test_f16_to_f64() {
        let f = f16::from_f64(7.0);
        assert_eq!(f.to_f64(), 7.0f64);

        // 7.1 is NOT exactly representable in 16-bit, it's rounded
        let f = f16::from_f64(7.1);
        let diff = (f.to_f64() - 7.1f64).abs();
        // diff must be <= 4 * EPSILON, as 7 has two more significant bits than 1
        assert!(diff <= 4.0 * f16::EPSILON.to_f64());

        assert_eq!(f16::from_bits(0x0000_0001).to_f64(), 2.0f64.powi(-24));
        assert_eq!(f16::from_bits(0x0000_0005).to_f64(), 5.0 * 2.0f64.powi(-24));

        assert_eq!(f16::from_bits(0x0000_0001), f16::from_f64(2.0f64.powi(-24)));
        assert_eq!(
            f16::from_bits(0x0000_0005),
            f16::from_f64(5.0 * 2.0f64.powi(-24))
        );
    }

    #[test]
    fn test_comparisons() {
        let zero = f16::from_f64(0.0);
        let one = f16::from_f64(1.0);
        let neg_zero = f16::from_f64(-0.0);
        let neg_one = f16::from_f64(-1.0);

        assert_eq!(zero.partial_cmp(&neg_zero), Some(Ordering::Equal));
        assert_eq!(neg_zero.partial_cmp(&zero), Some(Ordering::Equal));
        assert!(zero == neg_zero);
        assert!(neg_zero == zero);
        assert!(!(zero != neg_zero));
        assert!(!(neg_zero != zero));
        assert!(!(zero < neg_zero));
        assert!(!(neg_zero < zero));
        assert!(zero <= neg_zero);
        assert!(neg_zero <= zero);
        assert!(!(zero > neg_zero));
        assert!(!(neg_zero > zero));
        assert!(zero >= neg_zero);
        assert!(neg_zero >= zero);

        assert_eq!(one.partial_cmp(&neg_zero), Some(Ordering::Greater));
        assert_eq!(neg_zero.partial_cmp(&one), Some(Ordering::Less));
        assert!(!(one == neg_zero));
        assert!(!(neg_zero == one));
        assert!(one != neg_zero);
        assert!(neg_zero != one);
        assert!(!(one < neg_zero));
        assert!(neg_zero < one);
        assert!(!(one <= neg_zero));
        assert!(neg_zero <= one);
        assert!(one > neg_zero);
        assert!(!(neg_zero > one));
        assert!(one >= neg_zero);
        assert!(!(neg_zero >= one));

        assert_eq!(one.partial_cmp(&neg_one), Some(Ordering::Greater));
        assert_eq!(neg_one.partial_cmp(&one), Some(Ordering::Less));
        assert!(!(one == neg_one));
        assert!(!(neg_one == one));
        assert!(one != neg_one);
        assert!(neg_one != one);
        assert!(!(one < neg_one));
        assert!(neg_one < one);
        assert!(!(one <= neg_one));
        assert!(neg_one <= one);
        assert!(one > neg_one);
        assert!(!(neg_one > one));
        assert!(one >= neg_one);
        assert!(!(neg_one >= one));
    }

    #[test]
    #[allow(clippy::erasing_op, clippy::identity_op)]
    fn round_to_even_f32() {
        // smallest positive subnormal = 0b0.0000_0000_01 * 2^-14 = 2^-24
        let min_sub = f16::from_bits(1);
        let min_sub_f = (-24f32).exp2();
        assert_eq!(f16::from_f32(min_sub_f).to_bits(), min_sub.to_bits());
        assert_eq!(f32::from(min_sub).to_bits(), min_sub_f.to_bits());

        // 0.0000000000_011111 rounded to 0.0000000000 (< tie, no rounding)
        // 0.0000000000_100000 rounded to 0.0000000000 (tie and even, remains at even)
        // 0.0000000000_100001 rounded to 0.0000000001 (> tie, rounds up)
        assert_eq!(
            f16::from_f32(min_sub_f * 0.49).to_bits(),
            min_sub.to_bits() * 0
        );
        assert_eq!(
            f16::from_f32(min_sub_f * 0.50).to_bits(),
            min_sub.to_bits() * 0
        );
        assert_eq!(
            f16::from_f32(min_sub_f * 0.51).to_bits(),
            min_sub.to_bits() * 1
        );

        // 0.0000000001_011111 rounded to 0.0000000001 (< tie, no rounding)
        // 0.0000000001_100000 rounded to 0.0000000010 (tie and odd, rounds up to even)
        // 0.0000000001_100001 rounded to 0.0000000010 (> tie, rounds up)
        assert_eq!(
            f16::from_f32(min_sub_f * 1.49).to_bits(),
            min_sub.to_bits() * 1
        );
        assert_eq!(
            f16::from_f32(min_sub_f * 1.50).to_bits(),
            min_sub.to_bits() * 2
        );
        assert_eq!(
            f16::from_f32(min_sub_f * 1.51).to_bits(),
            min_sub.to_bits() * 2
        );

        // 0.0000000010_011111 rounded to 0.0000000010 (< tie, no rounding)
        // 0.0000000010_100000 rounded to 0.0000000010 (tie and even, remains at even)
        // 0.0000000010_100001 rounded to 0.0000000011 (> tie, rounds up)
        assert_eq!(
            f16::from_f32(min_sub_f * 2.49).to_bits(),
            min_sub.to_bits() * 2
        );
        assert_eq!(
            f16::from_f32(min_sub_f * 2.50).to_bits(),
            min_sub.to_bits() * 2
        );
        assert_eq!(
            f16::from_f32(min_sub_f * 2.51).to_bits(),
            min_sub.to_bits() * 3
        );

        assert_eq!(
            f16::from_f32(2000.49f32).to_bits(),
            f16::from_f32(2000.0).to_bits()
        );
        assert_eq!(
            f16::from_f32(2000.50f32).to_bits(),
            f16::from_f32(2000.0).to_bits()
        );
        assert_eq!(
            f16::from_f32(2000.51f32).to_bits(),
            f16::from_f32(2001.0).to_bits()
        );
        assert_eq!(
            f16::from_f32(2001.49f32).to_bits(),
            f16::from_f32(2001.0).to_bits()
        );
        assert_eq!(
            f16::from_f32(2001.50f32).to_bits(),
            f16::from_f32(2002.0).to_bits()
        );
        assert_eq!(
            f16::from_f32(2001.51f32).to_bits(),
            f16::from_f32(2002.0).to_bits()
        );
        assert_eq!(
            f16::from_f32(2002.49f32).to_bits(),
            f16::from_f32(2002.0).to_bits()
        );
        assert_eq!(
            f16::from_f32(2002.50f32).to_bits(),
            f16::from_f32(2002.0).to_bits()
        );
        assert_eq!(
            f16::from_f32(2002.51f32).to_bits(),
            f16::from_f32(2003.0).to_bits()
        );
    }

    #[test]
    #[allow(clippy::erasing_op, clippy::identity_op)]
    fn round_to_even_f64() {
        // smallest positive subnormal = 0b0.0000_0000_01 * 2^-14 = 2^-24
        let min_sub = f16::from_bits(1);
        let min_sub_f = (-24f64).exp2();
        assert_eq!(f16::from_f64(min_sub_f).to_bits(), min_sub.to_bits());
        assert_eq!(f64::from(min_sub).to_bits(), min_sub_f.to_bits());

        // 0.0000000000_011111 rounded to 0.0000000000 (< tie, no rounding)
        // 0.0000000000_100000 rounded to 0.0000000000 (tie and even, remains at even)
        // 0.0000000000_100001 rounded to 0.0000000001 (> tie, rounds up)
        assert_eq!(
            f16::from_f64(min_sub_f * 0.49).to_bits(),
            min_sub.to_bits() * 0
        );
        assert_eq!(
            f16::from_f64(min_sub_f * 0.50).to_bits(),
            min_sub.to_bits() * 0
        );
        assert_eq!(
            f16::from_f64(min_sub_f * 0.51).to_bits(),
            min_sub.to_bits() * 1
        );

        // 0.0000000001_011111 rounded to 0.0000000001 (< tie, no rounding)
        // 0.0000000001_100000 rounded to 0.0000000010 (tie and odd, rounds up to even)
        // 0.0000000001_100001 rounded to 0.0000000010 (> tie, rounds up)
        assert_eq!(
            f16::from_f64(min_sub_f * 1.49).to_bits(),
            min_sub.to_bits() * 1
        );
        assert_eq!(
            f16::from_f64(min_sub_f * 1.50).to_bits(),
            min_sub.to_bits() * 2
        );
        assert_eq!(
            f16::from_f64(min_sub_f * 1.51).to_bits(),
            min_sub.to_bits() * 2
        );

        // 0.0000000010_011111 rounded to 0.0000000010 (< tie, no rounding)
        // 0.0000000010_100000 rounded to 0.0000000010 (tie and even, remains at even)
        // 0.0000000010_100001 rounded to 0.0000000011 (> tie, rounds up)
        assert_eq!(
            f16::from_f64(min_sub_f * 2.49).to_bits(),
            min_sub.to_bits() * 2
        );
        assert_eq!(
            f16::from_f64(min_sub_f * 2.50).to_bits(),
            min_sub.to_bits() * 2
        );
        assert_eq!(
            f16::from_f64(min_sub_f * 2.51).to_bits(),
            min_sub.to_bits() * 3
        );

        assert_eq!(
            f16::from_f64(2000.49f64).to_bits(),
            f16::from_f64(2000.0).to_bits()
        );
        assert_eq!(
            f16::from_f64(2000.50f64).to_bits(),
            f16::from_f64(2000.0).to_bits()
        );
        assert_eq!(
            f16::from_f64(2000.51f64).to_bits(),
            f16::from_f64(2001.0).to_bits()
        );
        assert_eq!(
            f16::from_f64(2001.49f64).to_bits(),
            f16::from_f64(2001.0).to_bits()
        );
        assert_eq!(
            f16::from_f64(2001.50f64).to_bits(),
            f16::from_f64(2002.0).to_bits()
        );
        assert_eq!(
            f16::from_f64(2001.51f64).to_bits(),
            f16::from_f64(2002.0).to_bits()
        );
        assert_eq!(
            f16::from_f64(2002.49f64).to_bits(),
            f16::from_f64(2002.0).to_bits()
        );
        assert_eq!(
            f16::from_f64(2002.50f64).to_bits(),
            f16::from_f64(2002.0).to_bits()
        );
        assert_eq!(
            f16::from_f64(2002.51f64).to_bits(),
            f16::from_f64(2003.0).to_bits()
        );
    }

    #[test]
    fn arithmetic() {
        assert_eq!(f16::ONE + f16::ONE, f16::from_f32(2.));
        assert_eq!(f16::ONE - f16::ONE, f16::ZERO);
        assert_eq!(f16::ONE * f16::ONE, f16::ONE);
        assert_eq!(f16::from_f32(2.) * f16::from_f32(2.), f16::from_f32(4.));
        assert_eq!(f16::ONE / f16::ONE, f16::ONE);
        assert_eq!(f16::from_f32(4.) / f16::from_f32(2.), f16::from_f32(2.));
        assert_eq!(f16::from_f32(4.) % f16::from_f32(3.), f16::from_f32(1.));
    }

    #[cfg(feature = "std")]
    #[test]
    fn formatting() {
        let f = f16::from_f32(0.1152344);

        assert_eq!(format!("{:.3}", f), "0.115");
        assert_eq!(format!("{:.4}", f), "0.1152");
        assert_eq!(format!("{:+.4}", f), "+0.1152");
        assert_eq!(format!("{:>+10.4}", f), "   +0.1152");

        assert_eq!(format!("{:.3?}", f), "0.115");
        assert_eq!(format!("{:.4?}", f), "0.1152");
        assert_eq!(format!("{:+.4?}", f), "+0.1152");
        assert_eq!(format!("{:>+10.4?}", f), "   +0.1152");
    }

    impl quickcheck::Arbitrary for f16 {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            f16(u16::arbitrary(g))
        }
    }

    #[quickcheck]
    fn qc_roundtrip_f16_f32_is_identity(f: f16) -> bool {
        let roundtrip = f16::from_f32(f.to_f32());
        if f.is_nan() {
            roundtrip.is_nan() && f.is_sign_negative() == roundtrip.is_sign_negative()
        } else {
            f.0 == roundtrip.0
        }
    }

    #[quickcheck]
    fn qc_roundtrip_f16_f64_is_identity(f: f16) -> bool {
        let roundtrip = f16::from_f64(f.to_f64());
        if f.is_nan() {
            roundtrip.is_nan() && f.is_sign_negative() == roundtrip.is_sign_negative()
        } else {
            f.0 == roundtrip.0
        }
    }
}
