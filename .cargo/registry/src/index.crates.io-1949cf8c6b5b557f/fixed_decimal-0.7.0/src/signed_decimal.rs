// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::fmt;
use core::ops::{Deref, DerefMut};
use core::str::FromStr;

use crate::uint_iterator::IntIterator;
use crate::{variations::Signed, UnsignedDecimal};
#[cfg(feature = "ryu")]
use crate::{FloatPrecision, LimitError};
use crate::{
    IncrementLike, NoIncrement, ParseError, RoundingIncrement, Sign, SignDisplay,
    SignedRoundingMode, UnsignedRoundingMode,
};

/// A Type containing a [`UnsignedDecimal`] and a [`Sign`] to represent a signed decimal number.
///
/// Supports a mantissa of non-zero digits and a number of leading and trailing
/// zeros, as well as an optional sign; used for formatting and plural selection.
///
/// # Data Types
///
/// The following types can be converted to a `Decimal`:
///
/// - Integers, signed and unsigned
/// - Strings representing an arbitrary-precision decimal
/// - Floating point values (using the `ryu` feature)
///
/// To create a [`Decimal`] with fractional digits, you have several options:
/// - Create it from an integer and then call [`UnsignedDecimal::multiply_pow10`] (you can also call `multiply_pow10` directly on the [`Decimal`]).
/// - Create it from a string.
/// - When the `ryu` feature is enabled, create it from a floating point value using [`Decimal::try_from_f64`].
///
/// # Examples
///
/// ```
/// use fixed_decimal::Decimal;
///
/// let mut dec = Decimal::from(250);
/// assert_eq!("250", dec.to_string());
///
/// dec.multiply_pow10(-2);
/// assert_eq!("2.50", dec.to_string());
/// ```
pub type Decimal = Signed<UnsignedDecimal>;

impl Decimal {
    pub fn new(sign: Sign, absolute: UnsignedDecimal) -> Self {
        Decimal { sign, absolute }
    }

    #[inline]
    /// Parses a [`Decimal`].
    pub fn try_from_str(s: &str) -> Result<Self, ParseError> {
        Self::try_from_utf8(s.as_bytes())
    }

    pub fn try_from_utf8(input_str: &[u8]) -> Result<Self, ParseError> {
        // input_str: the input string
        // no_sign_str: the input string when the sign is removed from it
        if input_str.is_empty() {
            return Err(ParseError::Syntax);
        }
        #[allow(clippy::indexing_slicing)] // The string is not empty.
        let sign = match input_str[0] {
            b'-' => Sign::Negative,
            b'+' => Sign::Positive,
            _ => Sign::None,
        };
        #[allow(clippy::indexing_slicing)] // The string is not empty.
        let no_sign_str = if sign == Sign::None {
            input_str
        } else {
            &input_str[1..]
        };
        if no_sign_str.is_empty() {
            return Err(ParseError::Syntax);
        }

        let unsigned_decimal = UnsignedDecimal::try_from_no_sign_utf8(no_sign_str)?;
        Ok(Self {
            sign,
            absolute: unsigned_decimal,
        })
    }

    // TODO: Shall we add this method to `Signed<T>`?
    /// Sets the sign of this number according to the given sign display strategy.
    ///
    /// # Examples
    /// ```
    /// use fixed_decimal::Decimal;
    /// use fixed_decimal::SignDisplay::*;
    ///
    /// let mut dec = Decimal::from(1729);
    /// assert_eq!("1729", dec.to_string());
    /// dec.apply_sign_display(Always);
    /// assert_eq!("+1729", dec.to_string());
    /// ```
    pub fn apply_sign_display(&mut self, sign_display: SignDisplay) {
        use Sign::*;
        match sign_display {
            SignDisplay::Auto => {
                if self.sign != Negative {
                    self.sign = None
                }
            }
            SignDisplay::Always => {
                if self.sign != Negative {
                    self.sign = Positive
                }
            }
            SignDisplay::Never => self.sign = None,
            SignDisplay::ExceptZero => {
                if self.absolute.is_zero() {
                    self.sign = None
                } else if self.sign != Negative {
                    self.sign = Positive
                }
            }
            SignDisplay::Negative => {
                if self.sign != Negative || self.absolute.is_zero() {
                    self.sign = None
                }
            }
        }
    }

    // TODO: Shall we add this method to `Signed<T>`?
    /// Returns this number with its sign set according to the given sign display strategy.
    ///
    /// # Examples
    /// ```
    /// use fixed_decimal::Decimal;
    /// use fixed_decimal::SignDisplay::*;
    ///
    /// assert_eq!(
    ///     "+1729",
    ///     Decimal::from(1729)
    ///         .with_sign_display(ExceptZero)
    ///         .to_string()
    /// );
    /// ```
    pub fn with_sign_display(mut self, sign_display: SignDisplay) -> Self {
        self.apply_sign_display(sign_display);
        self
    }
}

impl FromStr for Decimal {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_str(s)
    }
}

macro_rules! impl_from_signed_integer_type {
    ($itype:ident, $utype: ident) => {
        impl From<$itype> for Decimal {
            fn from(value: $itype) -> Self {
                let int_iterator: IntIterator<$utype> = value.into();
                let sign = if int_iterator.is_negative {
                    Sign::Negative
                } else {
                    Sign::None
                };
                let value = UnsignedDecimal::from_ascending(int_iterator)
                    .expect("All built-in integer types should fit");
                Decimal {
                    sign,
                    absolute: value,
                }
            }
        }
    };
}

impl_from_signed_integer_type!(isize, usize);
impl_from_signed_integer_type!(i128, u128);
impl_from_signed_integer_type!(i64, u64);
impl_from_signed_integer_type!(i32, u32);
impl_from_signed_integer_type!(i16, u16);
impl_from_signed_integer_type!(i8, u8);

macro_rules! impl_from_unsigned_integer_type {
    ($utype: ident) => {
        impl From<$utype> for Decimal {
            fn from(value: $utype) -> Self {
                let int_iterator: IntIterator<$utype> = value.into();
                Self {
                    sign: Sign::None,
                    absolute: UnsignedDecimal::from_ascending(int_iterator)
                        .expect("All built-in integer types should fit"),
                }
            }
        }
    };
}

impl_from_unsigned_integer_type!(usize);
impl_from_unsigned_integer_type!(u128);
impl_from_unsigned_integer_type!(u64);
impl_from_unsigned_integer_type!(u32);
impl_from_unsigned_integer_type!(u16);
impl_from_unsigned_integer_type!(u8);

#[cfg(feature = "ryu")]
impl Decimal {
    /// Constructs a [`Decimal`] from an f64.
    ///
    /// Since f64 values do not carry a notion of their precision, the second argument to this
    /// function specifies the type of precision associated with the f64. For more information,
    /// see [`FloatPrecision`].
    ///
    /// This function uses `ryu`, which is an efficient double-to-string algorithm, but other
    /// implementations may yield higher performance; for more details, see
    /// [icu4x#166](https://github.com/unicode-org/icu4x/issues/166).
    ///
    /// This function can be made available with the `"ryu"` Cargo feature.
    ///
    /// ```rust
    /// use fixed_decimal::{Decimal, FloatPrecision};
    /// use writeable::assert_writeable_eq;
    ///
    /// let decimal = Decimal::try_from_f64(-5.1, FloatPrecision::Magnitude(-2))
    ///     .expect("Finite quantity with limited precision");
    /// assert_writeable_eq!(decimal, "-5.10");
    ///
    /// let decimal = Decimal::try_from_f64(0.012345678, FloatPrecision::RoundTrip)
    ///     .expect("Finite quantity");
    /// assert_writeable_eq!(decimal, "0.012345678");
    ///
    /// let decimal = Decimal::try_from_f64(12345678000., FloatPrecision::Integer)
    ///     .expect("Finite, integer-valued quantity");
    /// assert_writeable_eq!(decimal, "12345678000");
    /// ```
    ///
    /// Negative zero is supported.
    ///
    /// ```rust
    /// use fixed_decimal::{Decimal, FloatPrecision};
    /// use writeable::assert_writeable_eq;
    ///
    /// // IEEE 754 for floating point defines the sign bit separate
    /// // from the mantissa and exponent, allowing for -0.
    /// let negative_zero = Decimal::try_from_f64(-0.0, FloatPrecision::Integer)
    ///     .expect("Negative zero");
    /// assert_writeable_eq!(negative_zero, "-0");
    /// ```
    pub fn try_from_f64(float: f64, precision: FloatPrecision) -> Result<Self, LimitError> {
        match float.is_sign_negative() {
            true => Ok(Decimal {
                sign: Sign::Negative,
                absolute: UnsignedDecimal::try_from_f64(-float, precision)?,
            }),
            false => Ok(Decimal {
                sign: Sign::None,
                absolute: UnsignedDecimal::try_from_f64(float, precision)?,
            }),
        }
    }
}

impl Deref for Decimal {
    type Target = UnsignedDecimal;
    fn deref(&self) -> &Self::Target {
        &self.absolute
    }
}

impl DerefMut for Decimal {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.absolute
    }
}

/// All the rounding and rounding related logic is implmented in this implmentation block.
impl Decimal {
    /// Rounds this number at a particular digit position.
    ///
    /// This uses half to even rounding, which rounds to the nearest integer and resolves ties by
    /// selecting the nearest even integer to the original value.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    ///
    /// let mut dec = Decimal::from_str("-1.5").unwrap();
    /// dec.round(0);
    /// assert_eq!("-2", dec.to_string());
    /// let mut dec = Decimal::from_str("0.4").unwrap();
    /// dec.round(0);
    /// assert_eq!("0", dec.to_string());
    /// let mut dec = Decimal::from_str("0.5").unwrap();
    /// dec.round(0);
    /// assert_eq!("0", dec.to_string());
    /// let mut dec = Decimal::from_str("0.6").unwrap();
    /// dec.round(0);
    /// assert_eq!("1", dec.to_string());
    /// let mut dec = Decimal::from_str("1.5").unwrap();
    /// dec.round(0);
    /// assert_eq!("2", dec.to_string());
    /// ```
    pub fn round(&mut self, position: i16) {
        self.half_even_to_increment_internal(position, NoIncrement)
    }

    /// Returns this number rounded at a particular digit position.
    ///
    /// This uses half to even rounding by default, which rounds to the nearest integer and
    /// resolves ties by selecting the nearest even integer to the original value.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    ///
    /// let mut dec = Decimal::from_str("-1.5").unwrap();
    /// assert_eq!("-2", dec.rounded(0).to_string());
    /// let mut dec = Decimal::from_str("0.4").unwrap();
    /// assert_eq!("0", dec.rounded(0).to_string());
    /// let mut dec = Decimal::from_str("0.5").unwrap();
    /// assert_eq!("0", dec.rounded(0).to_string());
    /// let mut dec = Decimal::from_str("0.6").unwrap();
    /// assert_eq!("1", dec.rounded(0).to_string());
    /// let mut dec = Decimal::from_str("1.5").unwrap();
    /// assert_eq!("2", dec.rounded(0).to_string());
    /// ```
    pub fn rounded(mut self, position: i16) -> Self {
        self.round(position);
        self
    }

    /// Rounds this number towards positive infinity at a particular digit position.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    ///
    /// let mut dec = Decimal::from_str("-1.5").unwrap();
    /// dec.ceil(0);
    /// assert_eq!("-1", dec.to_string());
    /// let mut dec = Decimal::from_str("0.4").unwrap();
    /// dec.ceil(0);
    /// assert_eq!("1", dec.to_string());
    /// let mut dec = Decimal::from_str("0.5").unwrap();
    /// dec.ceil(0);
    /// assert_eq!("1", dec.to_string());
    /// let mut dec = Decimal::from_str("0.6").unwrap();
    /// dec.ceil(0);
    /// assert_eq!("1", dec.to_string());
    /// let mut dec = Decimal::from_str("1.5").unwrap();
    /// dec.ceil(0);
    /// assert_eq!("2", dec.to_string());
    /// ```
    #[inline(never)]
    pub fn ceil(&mut self, position: i16) {
        self.ceil_to_increment_internal(position, NoIncrement);
    }

    /// Returns this number rounded towards positive infinity at a particular digit position.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    ///
    /// let dec = Decimal::from_str("-1.5").unwrap();
    /// assert_eq!("-1", dec.ceiled(0).to_string());
    /// let dec = Decimal::from_str("0.4").unwrap();
    /// assert_eq!("1", dec.ceiled(0).to_string());
    /// let dec = Decimal::from_str("0.5").unwrap();
    /// assert_eq!("1", dec.ceiled(0).to_string());
    /// let dec = Decimal::from_str("0.6").unwrap();
    /// assert_eq!("1", dec.ceiled(0).to_string());
    /// let dec = Decimal::from_str("1.5").unwrap();
    /// assert_eq!("2", dec.ceiled(0).to_string());
    /// ```
    pub fn ceiled(mut self, position: i16) -> Self {
        self.ceil(position);
        self
    }

    /// Rounds this number away from zero at a particular digit position.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    ///
    /// let mut dec = Decimal::from_str("-1.5").unwrap();
    /// dec.expand(0);
    /// assert_eq!("-2", dec.to_string());
    /// let mut dec = Decimal::from_str("0.4").unwrap();
    /// dec.expand(0);
    /// assert_eq!("1", dec.to_string());
    /// let mut dec = Decimal::from_str("0.5").unwrap();
    /// dec.expand(0);
    /// assert_eq!("1", dec.to_string());
    /// let mut dec = Decimal::from_str("0.6").unwrap();
    /// dec.expand(0);
    /// assert_eq!("1", dec.to_string());
    /// let mut dec = Decimal::from_str("1.5").unwrap();
    /// dec.expand(0);
    /// assert_eq!("2", dec.to_string());
    /// ```
    #[inline(never)]
    pub fn expand(&mut self, position: i16) {
        self.expand_to_increment_internal(position, NoIncrement)
    }

    /// Returns this number rounded away from zero at a particular digit position.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    ///
    /// let dec = Decimal::from_str("-1.5").unwrap();
    /// assert_eq!("-2", dec.expanded(0).to_string());
    /// let dec = Decimal::from_str("0.4").unwrap();
    /// assert_eq!("1", dec.expanded(0).to_string());
    /// let dec = Decimal::from_str("0.5").unwrap();
    /// assert_eq!("1", dec.expanded(0).to_string());
    /// let dec = Decimal::from_str("0.6").unwrap();
    /// assert_eq!("1", dec.expanded(0).to_string());
    /// let dec = Decimal::from_str("1.5").unwrap();
    /// assert_eq!("2", dec.expanded(0).to_string());
    /// ```
    pub fn expanded(mut self, position: i16) -> Self {
        self.expand(position);
        self
    }

    /// Rounds this number towards negative infinity at a particular digit position.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    ///
    /// let mut dec = Decimal::from_str("-1.5").unwrap();
    /// dec.floor(0);
    /// assert_eq!("-2", dec.to_string());
    /// let mut dec = Decimal::from_str("0.4").unwrap();
    /// dec.floor(0);
    /// assert_eq!("0", dec.to_string());
    /// let mut dec = Decimal::from_str("0.5").unwrap();
    /// dec.floor(0);
    /// assert_eq!("0", dec.to_string());
    /// let mut dec = Decimal::from_str("0.6").unwrap();
    /// dec.floor(0);
    /// assert_eq!("0", dec.to_string());
    /// let mut dec = Decimal::from_str("1.5").unwrap();
    /// dec.floor(0);
    /// assert_eq!("1", dec.to_string());
    /// ```
    #[inline(never)]
    pub fn floor(&mut self, position: i16) {
        self.floor_to_increment_internal(position, NoIncrement);
    }

    /// Returns this number rounded towards negative infinity at a particular digit position.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    ///
    /// let dec = Decimal::from_str("-1.5").unwrap();
    /// assert_eq!("-2", dec.floored(0).to_string());
    /// let dec = Decimal::from_str("0.4").unwrap();
    /// assert_eq!("0", dec.floored(0).to_string());
    /// let dec = Decimal::from_str("0.5").unwrap();
    /// assert_eq!("0", dec.floored(0).to_string());
    /// let dec = Decimal::from_str("0.6").unwrap();
    /// assert_eq!("0", dec.floored(0).to_string());
    /// let dec = Decimal::from_str("1.5").unwrap();
    /// assert_eq!("1", dec.floored(0).to_string());
    /// ```
    pub fn floored(mut self, position: i16) -> Self {
        self.floor(position);
        self
    }

    /// Rounds this number towards zero at a particular digit position.
    ///
    /// Also see [`UnsignedDecimal::pad_end()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    ///
    /// let mut dec = Decimal::from_str("-1.5").unwrap();
    /// dec.trunc(0);
    /// assert_eq!("-1", dec.to_string());
    /// let mut dec = Decimal::from_str("0.4").unwrap();
    /// dec.trunc(0);
    /// assert_eq!("0", dec.to_string());
    /// let mut dec = Decimal::from_str("0.5").unwrap();
    /// dec.trunc(0);
    /// assert_eq!("0", dec.to_string());
    /// let mut dec = Decimal::from_str("0.6").unwrap();
    /// dec.trunc(0);
    /// assert_eq!("0", dec.to_string());
    /// let mut dec = Decimal::from_str("1.5").unwrap();
    /// dec.trunc(0);
    /// assert_eq!("1", dec.to_string());
    /// ```
    #[inline(never)]
    pub fn trunc(&mut self, position: i16) {
        self.trunc_to_increment_internal(position, NoIncrement)
    }

    /// Returns this number rounded towards zero at a particular digit position.
    ///
    /// Also see [`UnsignedDecimal::padded_end()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    ///
    /// let dec = Decimal::from_str("-1.5").unwrap();
    /// assert_eq!("-1", dec.trunced(0).to_string());
    /// let dec = Decimal::from_str("0.4").unwrap();
    /// assert_eq!("0", dec.trunced(0).to_string());
    /// let dec = Decimal::from_str("0.5").unwrap();
    /// assert_eq!("0", dec.trunced(0).to_string());
    /// let dec = Decimal::from_str("0.6").unwrap();
    /// assert_eq!("0", dec.trunced(0).to_string());
    /// let dec = Decimal::from_str("1.5").unwrap();
    /// assert_eq!("1", dec.trunced(0).to_string());
    /// ```
    pub fn trunced(mut self, position: i16) -> Self {
        self.trunc(position);
        self
    }

    /// Rounds this number at a particular digit position, using the specified rounding mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::{Decimal, SignedRoundingMode, UnsignedRoundingMode};
    /// # use std::str::FromStr;
    ///
    /// let mut dec = Decimal::from_str("-3.5").unwrap();
    /// dec.round_with_mode(0, SignedRoundingMode::Floor);
    /// assert_eq!("-4", dec.to_string());
    /// let mut dec = Decimal::from_str("-3.5").unwrap();
    /// dec.round_with_mode(0, SignedRoundingMode::Ceil);
    /// assert_eq!("-3", dec.to_string());
    /// let mut dec = Decimal::from_str("5.455").unwrap();
    /// dec.round_with_mode(
    ///     -2,
    ///     SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    /// );
    /// assert_eq!("5.46", dec.to_string());
    /// let mut dec = Decimal::from_str("-7.235").unwrap();
    /// dec.round_with_mode(
    ///     -2,
    ///     SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
    /// );
    /// assert_eq!("-7.23", dec.to_string());
    /// let mut dec = Decimal::from_str("9.75").unwrap();
    /// dec.round_with_mode(
    ///     -1,
    ///     SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
    /// );
    /// assert_eq!("9.8", dec.to_string());
    /// ```
    pub fn round_with_mode(&mut self, position: i16, mode: SignedRoundingMode) {
        match mode {
            SignedRoundingMode::Ceil => self.ceil_to_increment_internal(position, NoIncrement),
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand) => {
                self.expand_to_increment_internal(position, NoIncrement)
            }
            SignedRoundingMode::Floor => self.floor_to_increment_internal(position, NoIncrement),
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc) => {
                self.trunc_to_increment_internal(position, NoIncrement)
            }
            SignedRoundingMode::HalfCeil => {
                self.half_ceil_to_increment_internal(position, NoIncrement)
            }
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand) => {
                self.half_expand_to_increment_internal(position, NoIncrement)
            }
            SignedRoundingMode::HalfFloor => {
                self.half_floor_to_increment_internal(position, NoIncrement)
            }
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc) => {
                self.half_trunc_to_increment_internal(position, NoIncrement)
            }
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven) => {
                self.half_even_to_increment_internal(position, NoIncrement)
            }
        }
    }

    /// Returns this number rounded at a particular digit position, using the specified rounding mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::{Decimal, SignedRoundingMode, UnsignedRoundingMode};
    /// # use std::str::FromStr;
    ///
    /// let mut dec = Decimal::from_str("-3.5").unwrap();
    /// assert_eq!(
    ///     "-4",
    ///     dec.rounded_with_mode(0, SignedRoundingMode::Floor)
    ///         .to_string()
    /// );
    /// let mut dec = Decimal::from_str("-3.5").unwrap();
    /// assert_eq!(
    ///     "-3",
    ///     dec.rounded_with_mode(0, SignedRoundingMode::Ceil)
    ///         .to_string()
    /// );
    /// let mut dec = Decimal::from_str("5.455").unwrap();
    /// assert_eq!(
    ///     "5.46",
    ///     dec.rounded_with_mode(
    ///         -2,
    ///         SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand)
    ///     )
    ///     .to_string()
    /// );
    /// let mut dec = Decimal::from_str("-7.235").unwrap();
    /// assert_eq!(
    ///     "-7.23",
    ///     dec.rounded_with_mode(
    ///         -2,
    ///         SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc)
    ///     )
    ///     .to_string()
    /// );
    /// let mut dec = Decimal::from_str("9.75").unwrap();
    /// assert_eq!(
    ///     "9.8",
    ///     dec.rounded_with_mode(
    ///         -1,
    ///         SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven)
    ///     )
    ///     .to_string()
    /// );
    /// ```
    pub fn rounded_with_mode(mut self, position: i16, mode: SignedRoundingMode) -> Self {
        self.round_with_mode(position, mode);
        self
    }

    /// Rounds this number at a particular digit position and increment, using the specified rounding mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::{
    ///     Decimal, RoundingIncrement, SignedRoundingMode, UnsignedRoundingMode,
    /// };
    /// # use std::str::FromStr;
    ///
    /// let mut dec = Decimal::from_str("-3.5").unwrap();
    /// dec.round_with_mode_and_increment(
    ///     0,
    ///     SignedRoundingMode::Floor,
    ///     RoundingIncrement::MultiplesOf1,
    /// );
    /// assert_eq!("-4", dec.to_string());
    /// let mut dec = Decimal::from_str("-3.59").unwrap();
    /// dec.round_with_mode_and_increment(
    ///     -1,
    ///     SignedRoundingMode::Ceil,
    ///     RoundingIncrement::MultiplesOf2,
    /// );
    /// assert_eq!("-3.4", dec.to_string());
    /// let mut dec = Decimal::from_str("5.455").unwrap();
    /// dec.round_with_mode_and_increment(
    ///     -2,
    ///     SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    ///     RoundingIncrement::MultiplesOf5,
    /// );
    /// assert_eq!("5.45", dec.to_string());
    /// let mut dec = Decimal::from_str("-7.235").unwrap();
    /// dec.round_with_mode_and_increment(
    ///     -2,
    ///     SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
    ///     RoundingIncrement::MultiplesOf25,
    /// );
    /// assert_eq!("-7.25", dec.to_string());
    /// let mut dec = Decimal::from_str("9.75").unwrap();
    /// dec.round_with_mode_and_increment(
    ///     -1,
    ///     SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
    ///     RoundingIncrement::MultiplesOf5,
    /// );
    /// assert_eq!("10.0", dec.to_string());
    /// ```
    pub fn round_with_mode_and_increment(
        &mut self,
        position: i16,
        mode: SignedRoundingMode,
        increment: RoundingIncrement,
    ) {
        match mode {
            SignedRoundingMode::Ceil => self.ceil_to_increment_internal(position, increment),
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand) => {
                self.expand_to_increment_internal(position, increment)
            }
            SignedRoundingMode::Floor => self.floor_to_increment_internal(position, increment),
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc) => {
                self.trunc_to_increment_internal(position, increment)
            }
            SignedRoundingMode::HalfCeil => {
                self.half_ceil_to_increment_internal(position, increment)
            }
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand) => {
                self.half_expand_to_increment_internal(position, increment)
            }
            SignedRoundingMode::HalfFloor => {
                self.half_floor_to_increment_internal(position, increment)
            }
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc) => {
                self.half_trunc_to_increment_internal(position, increment)
            }
            SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven) => {
                self.half_even_to_increment_internal(position, increment)
            }
        }
    }

    /// Returns this number rounded at a particular digit position and increment, using the specified rounding mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use fixed_decimal::{
    ///     Decimal, RoundingIncrement, SignedRoundingMode, UnsignedRoundingMode,
    /// };
    /// # use std::str::FromStr;
    ///
    /// let mut dec = Decimal::from_str("-3.5").unwrap();
    /// assert_eq!(
    ///     "-4",
    ///     dec.rounded_with_mode_and_increment(
    ///         0,
    ///         SignedRoundingMode::Floor,
    ///         RoundingIncrement::MultiplesOf1
    ///     )
    ///     .to_string()
    /// );
    /// let mut dec = Decimal::from_str("-3.59").unwrap();
    /// assert_eq!(
    ///     "-3.4",
    ///     dec.rounded_with_mode_and_increment(
    ///         -1,
    ///         SignedRoundingMode::Ceil,
    ///         RoundingIncrement::MultiplesOf2
    ///     )
    ///     .to_string()
    /// );
    /// let mut dec = Decimal::from_str("5.455").unwrap();
    /// assert_eq!(
    ///     "5.45",
    ///     dec.rounded_with_mode_and_increment(
    ///         -2,
    ///         SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    ///         RoundingIncrement::MultiplesOf5
    ///     )
    ///     .to_string()
    /// );
    /// let mut dec = Decimal::from_str("-7.235").unwrap();
    /// assert_eq!(
    ///     "-7.25",
    ///     dec.rounded_with_mode_and_increment(
    ///         -2,
    ///         SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
    ///         RoundingIncrement::MultiplesOf25
    ///     )
    ///     .to_string()
    /// );
    /// let mut dec = Decimal::from_str("9.75").unwrap();
    /// assert_eq!(
    ///     "10.0",
    ///     dec.rounded_with_mode_and_increment(
    ///         -1,
    ///         SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
    ///         RoundingIncrement::MultiplesOf5
    ///     )
    ///     .to_string()
    /// );
    /// ```
    pub fn rounded_with_mode_and_increment(
        mut self,
        position: i16,
        mode: SignedRoundingMode,
        increment: RoundingIncrement,
    ) -> Self {
        self.round_with_mode_and_increment(position, mode, increment);
        self
    }

    fn ceil_to_increment_internal<R: IncrementLike>(&mut self, position: i16, increment: R) {
        if self.sign == Sign::Negative {
            self.trunc_to_increment_internal(position, increment);
            return;
        }

        self.expand_to_increment_internal(position, increment);
    }

    fn half_ceil_to_increment_internal<R: IncrementLike>(&mut self, position: i16, increment: R) {
        if self.sign == Sign::Negative {
            self.half_trunc_to_increment_internal(position, increment);
            return;
        }

        self.half_expand_to_increment_internal(position, increment);
    }

    fn floor_to_increment_internal<R: IncrementLike>(&mut self, position: i16, increment: R) {
        if self.sign == Sign::Negative {
            self.expand_to_increment_internal(position, increment);
            return;
        }

        self.trunc_to_increment_internal(position, increment);
    }

    fn half_floor_to_increment_internal<R: IncrementLike>(&mut self, position: i16, increment: R) {
        if self.sign == Sign::Negative {
            self.half_expand_to_increment_internal(position, increment);
            return;
        }

        self.half_trunc_to_increment_internal(position, increment);
    }
}

/// Render the [`Decimal`] as a string of ASCII digits with a possible decimal point.
///
/// # Examples
///
/// ```
/// # use fixed_decimal::Decimal;
/// # use writeable::assert_writeable_eq;
/// #
/// assert_writeable_eq!(Decimal::from(42), "42");
/// ```
impl writeable::Writeable for Decimal {
    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        match self.sign {
            Sign::Negative => sink.write_char('-')?,
            Sign::Positive => sink.write_char('+')?,
            Sign::None => (),
        }
        for m in self.absolute.magnitude_range().rev() {
            if m == -1 {
                sink.write_char('.')?;
            }
            let d = self.absolute.digit_at(m);
            sink.write_char((b'0' + d) as char)?;
        }
        Ok(())
    }

    fn writeable_length_hint(&self) -> writeable::LengthHint {
        self.absolute.writeable_length_hint() + (self.sign != Sign::None) as usize
    }
}

writeable::impl_display_with_writeable!(Decimal);

#[test]
fn test_basic() {
    #[derive(Debug)]
    struct TestCase {
        pub input: isize,
        pub delta: i16,
        pub expected: &'static str,
    }
    let cases = [
        TestCase {
            input: 51423,
            delta: 0,
            expected: "51423",
        },
        TestCase {
            input: 51423,
            delta: -2,
            expected: "514.23",
        },
        TestCase {
            input: 51423,
            delta: -5,
            expected: "0.51423",
        },
        TestCase {
            input: 51423,
            delta: -8,
            expected: "0.00051423",
        },
        TestCase {
            input: 51423,
            delta: 3,
            expected: "51423000",
        },
        TestCase {
            input: 0,
            delta: 0,
            expected: "0",
        },
        TestCase {
            input: 0,
            delta: -2,
            expected: "0.00",
        },
        TestCase {
            input: 0,
            delta: 3,
            expected: "0000",
        },
        TestCase {
            input: 500,
            delta: 0,
            expected: "500",
        },
        TestCase {
            input: 500,
            delta: -1,
            expected: "50.0",
        },
        TestCase {
            input: 500,
            delta: -2,
            expected: "5.00",
        },
        TestCase {
            input: 500,
            delta: -3,
            expected: "0.500",
        },
        TestCase {
            input: 500,
            delta: -4,
            expected: "0.0500",
        },
        TestCase {
            input: 500,
            delta: 3,
            expected: "500000",
        },
        TestCase {
            input: -123,
            delta: 0,
            expected: "-123",
        },
        TestCase {
            input: -123,
            delta: -2,
            expected: "-1.23",
        },
        TestCase {
            input: -123,
            delta: -5,
            expected: "-0.00123",
        },
        TestCase {
            input: -123,
            delta: 3,
            expected: "-123000",
        },
    ];
    for cas in &cases {
        let mut dec: Decimal = cas.input.into();
        // println!("{}", cas.input + 0.01);
        dec.absolute.multiply_pow10(cas.delta);
        writeable::assert_writeable_eq!(dec, cas.expected, "{:?}", cas);
    }
}

#[test]
fn test_from_str() {
    #[derive(Debug)]
    struct TestCase {
        pub input_str: &'static str,
        /// The output str, `None` for roundtrip
        pub output_str: Option<&'static str>,
        /// [upper magnitude, upper nonzero magnitude, lower nonzero magnitude, lower magnitude]
        pub magnitudes: [i16; 4],
    }
    let cases = [
        TestCase {
            input_str: "-00123400",
            output_str: None,
            magnitudes: [7, 5, 2, 0],
        },
        TestCase {
            input_str: "+00123400",
            output_str: None,
            magnitudes: [7, 5, 2, 0],
        },
        TestCase {
            input_str: "0.0123400",
            output_str: None,
            magnitudes: [0, -2, -5, -7],
        },
        TestCase {
            input_str: "-00.123400",
            output_str: None,
            magnitudes: [1, -1, -4, -6],
        },
        TestCase {
            input_str: "0012.3400",
            output_str: None,
            magnitudes: [3, 1, -2, -4],
        },
        TestCase {
            input_str: "-0012340.0",
            output_str: None,
            magnitudes: [6, 4, 1, -1],
        },
        TestCase {
            input_str: "1234",
            output_str: None,
            magnitudes: [3, 3, 0, 0],
        },
        TestCase {
            input_str: "0.000000001",
            output_str: None,
            magnitudes: [0, -9, -9, -9],
        },
        TestCase {
            input_str: "0.0000000010",
            output_str: None,
            magnitudes: [0, -9, -9, -10],
        },
        TestCase {
            input_str: "1000000",
            output_str: None,
            magnitudes: [6, 6, 6, 0],
        },
        TestCase {
            input_str: "10000001",
            output_str: None,
            magnitudes: [7, 7, 0, 0],
        },
        TestCase {
            input_str: "123",
            output_str: None,
            magnitudes: [2, 2, 0, 0],
        },
        TestCase {
            input_str: "922337203685477580898230948203840239384.9823094820384023938423424",
            output_str: None,
            magnitudes: [38, 38, -25, -25],
        },
        TestCase {
            input_str: "009223372000.003685477580898230948203840239384000",
            output_str: None,
            magnitudes: [11, 9, -33, -36],
        },
        TestCase {
            input_str: "-009223372000.003685477580898230948203840239384000",
            output_str: None,
            magnitudes: [11, 9, -33, -36],
        },
        TestCase {
            input_str: "0",
            output_str: None,
            magnitudes: [0, 0, 0, 0],
        },
        TestCase {
            input_str: "-0",
            output_str: None,
            magnitudes: [0, 0, 0, 0],
        },
        TestCase {
            input_str: "+0",
            output_str: None,
            magnitudes: [0, 0, 0, 0],
        },
        TestCase {
            input_str: "000",
            output_str: None,
            magnitudes: [2, 0, 0, 0],
        },
        TestCase {
            input_str: "-00.0",
            output_str: None,
            magnitudes: [1, 0, 0, -1],
        },
        // no leading 0 parsing
        TestCase {
            input_str: ".0123400",
            output_str: Some("0.0123400"),
            magnitudes: [0, -2, -5, -7],
        },
        TestCase {
            input_str: ".000000001",
            output_str: Some("0.000000001"),
            magnitudes: [0, -9, -9, -9],
        },
        TestCase {
            input_str: "-.123400",
            output_str: Some("-0.123400"),
            magnitudes: [0, -1, -4, -6],
        },
    ];
    for cas in &cases {
        let fd = Decimal::from_str(cas.input_str).unwrap();
        assert_eq!(
            fd.absolute.magnitude_range(),
            cas.magnitudes[3]..=cas.magnitudes[0],
            "{cas:?}"
        );
        assert_eq!(
            fd.absolute.nonzero_magnitude_start(),
            cas.magnitudes[1],
            "{cas:?}"
        );
        assert_eq!(
            fd.absolute.nonzero_magnitude_end(),
            cas.magnitudes[2],
            "{cas:?}"
        );
        let input_str_roundtrip = fd.to_string();
        let output_str = cas.output_str.unwrap_or(cas.input_str);
        assert_eq!(output_str, input_str_roundtrip, "{cas:?}");
    }
}

#[test]
fn test_from_str_scientific() {
    #[derive(Debug)]
    struct TestCase {
        pub input_str: &'static str,
        pub output: &'static str,
    }
    let cases = [
        TestCase {
            input_str: "-5.4e10",
            output: "-54000000000",
        },
        TestCase {
            input_str: "5.4e-2",
            output: "0.054",
        },
        TestCase {
            input_str: "54.1e-2",
            output: "0.541",
        },
        TestCase {
            input_str: "-541e-2",
            output: "-5.41",
        },
        TestCase {
            input_str: "0.009E10",
            output: "90000000",
        },
        TestCase {
            input_str: "-9000E-10",
            output: "-0.0000009",
        },
    ];
    for cas in &cases {
        let input_str_roundtrip = Decimal::from_str(cas.input_str).unwrap().to_string();
        assert_eq!(cas.output, input_str_roundtrip);
    }
}

#[test]
fn test_isize_limits() {
    for num in &[isize::MAX, isize::MIN] {
        let dec: Decimal = (*num).into();
        let dec_str = dec.to_string();
        assert_eq!(num.to_string(), dec_str);
        assert_eq!(dec, Decimal::from_str(&dec_str).unwrap());
        writeable::assert_writeable_eq!(dec, dec_str);
    }
}

#[test]
fn test_ui128_limits() {
    for num in &[i128::MAX, i128::MIN] {
        let dec: Decimal = (*num).into();
        let dec_str = dec.to_string();
        assert_eq!(num.to_string(), dec_str);
        assert_eq!(dec, Decimal::from_str(&dec_str).unwrap());
        writeable::assert_writeable_eq!(dec, dec_str);
    }
    for num in &[u128::MAX, u128::MIN] {
        let dec: Decimal = (*num).into();
        let dec_str = dec.to_string();
        assert_eq!(num.to_string(), dec_str);
        assert_eq!(dec, Decimal::from_str(&dec_str).unwrap());
        writeable::assert_writeable_eq!(dec, dec_str);
    }
}

#[test]
fn test_zero_str_bounds() {
    #[derive(Debug)]
    struct TestCase {
        pub zeros_before_dot: usize,
        pub zeros_after_dot: usize,
        pub expected_err: Option<ParseError>,
    }
    let cases = [
        TestCase {
            zeros_before_dot: i16::MAX as usize + 1,
            zeros_after_dot: 0,
            expected_err: None,
        },
        TestCase {
            zeros_before_dot: i16::MAX as usize,
            zeros_after_dot: 0,
            expected_err: None,
        },
        TestCase {
            zeros_before_dot: i16::MAX as usize + 2,
            zeros_after_dot: 0,
            expected_err: Some(ParseError::Limit),
        },
        TestCase {
            zeros_before_dot: 0,
            zeros_after_dot: i16::MAX as usize + 2,
            expected_err: Some(ParseError::Limit),
        },
        TestCase {
            zeros_before_dot: i16::MAX as usize + 1,
            zeros_after_dot: i16::MAX as usize + 1,
            expected_err: None,
        },
        TestCase {
            zeros_before_dot: i16::MAX as usize + 2,
            zeros_after_dot: i16::MAX as usize + 1,
            expected_err: Some(ParseError::Limit),
        },
        TestCase {
            zeros_before_dot: i16::MAX as usize + 1,
            zeros_after_dot: i16::MAX as usize + 2,
            expected_err: Some(ParseError::Limit),
        },
        TestCase {
            zeros_before_dot: i16::MAX as usize,
            zeros_after_dot: i16::MAX as usize + 2,
            expected_err: Some(ParseError::Limit),
        },
        TestCase {
            zeros_before_dot: i16::MAX as usize,
            zeros_after_dot: i16::MAX as usize,
            expected_err: None,
        },
        TestCase {
            zeros_before_dot: i16::MAX as usize + 1,
            zeros_after_dot: i16::MAX as usize,
            expected_err: None,
        },
    ];
    for cas in &cases {
        let mut input_str = format!("{:0fill$}", 0, fill = cas.zeros_before_dot);
        if cas.zeros_after_dot > 0 {
            input_str.push('.');
            input_str.push_str(&format!("{:0fill$}", 0, fill = cas.zeros_after_dot));
        }
        match Decimal::from_str(&input_str) {
            Ok(dec) => {
                assert_eq!(cas.expected_err, None, "{cas:?}");
                assert_eq!(input_str, dec.to_string(), "{cas:?}");
            }
            Err(err) => {
                assert_eq!(cas.expected_err, Some(err), "{cas:?}");
            }
        }
    }
}

#[test]
fn test_syntax_error() {
    #[derive(Debug)]
    struct TestCase {
        pub input_str: &'static str,
        pub expected_err: Option<ParseError>,
    }
    let cases = [
        TestCase {
            input_str: "-12a34",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "0.0123√400",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "0.012.3400",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "-0-0123400",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "0-0123400",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "-0.00123400",
            expected_err: None,
        },
        TestCase {
            input_str: "00123400.",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "00123400.0",
            expected_err: None,
        },
        TestCase {
            input_str: "123_456",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "-",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "+",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "-1",
            expected_err: None,
        },
    ];
    for cas in &cases {
        match Decimal::from_str(cas.input_str) {
            Ok(dec) => {
                assert_eq!(cas.expected_err, None, "{cas:?}");
                assert_eq!(cas.input_str, dec.to_string(), "{cas:?}");
            }
            Err(err) => {
                assert_eq!(cas.expected_err, Some(err), "{cas:?}");
            }
        }
    }
}

#[test]
fn test_pad() {
    let mut dec = Decimal::from_str("-0.42").unwrap();
    assert_eq!("-0.42", dec.to_string());

    dec.absolute.pad_start(1);
    assert_eq!("-0.42", dec.to_string());

    dec.absolute.pad_start(4);
    assert_eq!("-0000.42", dec.to_string());

    dec.absolute.pad_start(2);
    assert_eq!("-00.42", dec.to_string());
}

#[test]
fn test_sign_display() {
    use SignDisplay::*;
    let positive_nonzero = Decimal::from(163u32);
    let negative_nonzero = Decimal::from(-163);
    let positive_zero = Decimal::from(0u32);
    let negative_zero = Decimal::from(0u32).with_sign(Sign::Negative);
    assert_eq!(
        "163",
        positive_nonzero.clone().with_sign_display(Auto).to_string()
    );
    assert_eq!(
        "-163",
        negative_nonzero.clone().with_sign_display(Auto).to_string()
    );
    assert_eq!(
        "0",
        positive_zero.clone().with_sign_display(Auto).to_string()
    );
    assert_eq!(
        "-0",
        negative_zero.clone().with_sign_display(Auto).to_string()
    );
    assert_eq!(
        "+163",
        positive_nonzero
            .clone()
            .with_sign_display(Always)
            .to_string()
    );
    assert_eq!(
        "-163",
        negative_nonzero
            .clone()
            .with_sign_display(Always)
            .to_string()
    );
    assert_eq!(
        "+0",
        positive_zero.clone().with_sign_display(Always).to_string()
    );
    assert_eq!(
        "-0",
        negative_zero.clone().with_sign_display(Always).to_string()
    );
    assert_eq!(
        "163",
        positive_nonzero
            .clone()
            .with_sign_display(Never)
            .to_string()
    );
    assert_eq!(
        "163",
        negative_nonzero
            .clone()
            .with_sign_display(Never)
            .to_string()
    );
    assert_eq!(
        "0",
        positive_zero.clone().with_sign_display(Never).to_string()
    );
    assert_eq!(
        "0",
        negative_zero.clone().with_sign_display(Never).to_string()
    );
    assert_eq!(
        "+163",
        positive_nonzero
            .clone()
            .with_sign_display(ExceptZero)
            .to_string()
    );
    assert_eq!(
        "-163",
        negative_nonzero
            .clone()
            .with_sign_display(ExceptZero)
            .to_string()
    );
    assert_eq!(
        "0",
        positive_zero
            .clone()
            .with_sign_display(ExceptZero)
            .to_string()
    );
    assert_eq!(
        "0",
        negative_zero
            .clone()
            .with_sign_display(ExceptZero)
            .to_string()
    );
    assert_eq!(
        "163",
        positive_nonzero.with_sign_display(Negative).to_string()
    );
    assert_eq!(
        "-163",
        negative_nonzero.with_sign_display(Negative).to_string()
    );
    assert_eq!("0", positive_zero.with_sign_display(Negative).to_string());
    assert_eq!("0", negative_zero.with_sign_display(Negative).to_string());
}

#[test]
fn test_set_max_position() {
    let mut dec = Decimal::from(1000u32);
    assert_eq!("1000", dec.to_string());

    dec.absolute.set_max_position(2);
    assert_eq!("00", dec.to_string());

    dec.absolute.set_max_position(0);
    assert_eq!("0", dec.to_string());

    dec.absolute.set_max_position(3);
    assert_eq!("000", dec.to_string());

    let mut dec = Decimal::from_str("0.456").unwrap();
    assert_eq!("0.456", dec.to_string());

    dec.absolute.set_max_position(0);
    assert_eq!("0.456", dec.to_string());

    dec.absolute.set_max_position(-1);
    assert_eq!("0.056", dec.to_string());

    dec.absolute.set_max_position(-2);
    assert_eq!("0.006", dec.to_string());

    dec.absolute.set_max_position(-3);
    assert_eq!("0.000", dec.to_string());

    dec.absolute.set_max_position(-4);
    assert_eq!("0.0000", dec.to_string());

    let mut dec = Decimal::from_str("100.01").unwrap();
    dec.absolute.set_max_position(1);
    assert_eq!("0.01", dec.to_string());
}

#[test]
fn test_pad_start_bounds() {
    let mut dec = Decimal::from_str("299792.458").unwrap();
    let max_integer_digits = i16::MAX as usize + 1;

    dec.absolute.pad_start(i16::MAX - 1);
    assert_eq!(
        max_integer_digits - 2,
        dec.to_string().split_once('.').unwrap().0.len()
    );

    dec.absolute.pad_start(i16::MAX);
    assert_eq!(
        max_integer_digits - 1,
        dec.to_string().split_once('.').unwrap().0.len()
    );
}

#[test]
fn test_pad_end_bounds() {
    let mut dec = Decimal::from_str("299792.458").unwrap();
    let max_fractional_digits = -(i16::MIN as isize) as usize;

    dec.absolute.pad_end(i16::MIN + 1);
    assert_eq!(
        max_fractional_digits - 1,
        dec.to_string().split_once('.').unwrap().1.len()
    );

    dec.absolute.pad_end(i16::MIN);
    assert_eq!(
        max_fractional_digits,
        dec.to_string().split_once('.').unwrap().1.len()
    );
}

#[test]
fn test_rounding() {
    pub(crate) use std::str::FromStr;

    // Test Ceil
    let mut dec = Decimal::from_str("3.234").unwrap();
    dec.ceil(0);
    assert_eq!("4", dec.to_string());

    let mut dec = Decimal::from_str("2.222").unwrap();
    dec.ceil(-1);
    assert_eq!("2.3", dec.to_string());

    let mut dec = Decimal::from_str("22.222").unwrap();
    dec.ceil(-2);
    assert_eq!("22.23", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.ceil(-2);
    assert_eq!("100.00", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.ceil(-5);
    assert_eq!("99.99900", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.ceil(-5);
    assert_eq!("-99.99900", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.ceil(-2);
    assert_eq!("-99.99", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.ceil(4);
    assert_eq!("10000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.ceil(4);
    assert_eq!("-0000", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.ceil(-1);
    assert_eq!("0.1", dec.to_string());

    let mut dec = Decimal::from_str("-0.009").unwrap();
    dec.ceil(-1);
    assert_eq!("-0.0", dec.to_string());

    // Test Half Ceil
    let mut dec = Decimal::from_str("3.234").unwrap();
    dec.round_with_mode(0, SignedRoundingMode::HalfCeil);
    assert_eq!("3", dec.to_string());

    let mut dec = Decimal::from_str("3.534").unwrap();
    dec.round_with_mode(0, SignedRoundingMode::HalfCeil);
    assert_eq!("4", dec.to_string());

    let mut dec = Decimal::from_str("3.934").unwrap();
    dec.round_with_mode(0, SignedRoundingMode::HalfCeil);
    assert_eq!("4", dec.to_string());

    let mut dec = Decimal::from_str("2.222").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfCeil);
    assert_eq!("2.2", dec.to_string());

    let mut dec = Decimal::from_str("2.44").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfCeil);
    assert_eq!("2.4", dec.to_string());

    let mut dec = Decimal::from_str("2.45").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfCeil);
    assert_eq!("2.5", dec.to_string());

    let mut dec = Decimal::from_str("-2.44").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfCeil);
    assert_eq!("-2.4", dec.to_string());

    let mut dec = Decimal::from_str("-2.45").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfCeil);
    assert_eq!("-2.4", dec.to_string());

    let mut dec = Decimal::from_str("22.222").unwrap();
    dec.round_with_mode(-2, SignedRoundingMode::HalfCeil);
    assert_eq!("22.22", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.round_with_mode(-2, SignedRoundingMode::HalfCeil);
    assert_eq!("100.00", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.round_with_mode(-5, SignedRoundingMode::HalfCeil);
    assert_eq!("99.99900", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode(-5, SignedRoundingMode::HalfCeil);
    assert_eq!("-99.99900", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode(-2, SignedRoundingMode::HalfCeil);
    assert_eq!("-100.00", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.round_with_mode(4, SignedRoundingMode::HalfCeil);
    assert_eq!("0000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode(4, SignedRoundingMode::HalfCeil);
    assert_eq!("-0000", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfCeil);
    assert_eq!("0.0", dec.to_string());

    let mut dec = Decimal::from_str("-0.009").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfCeil);
    assert_eq!("-0.0", dec.to_string());

    // Test Floor
    let mut dec = Decimal::from_str("3.234").unwrap();
    dec.floor(0);
    assert_eq!("3", dec.to_string());

    let mut dec = Decimal::from_str("2.222").unwrap();
    dec.floor(-1);
    assert_eq!("2.2", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.floor(-2);
    assert_eq!("99.99", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.floor(-10);
    assert_eq!("99.9990000000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.floor(-10);
    assert_eq!("-99.9990000000", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.floor(10);
    assert_eq!("0000000000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.floor(10);
    assert_eq!("-10000000000", dec.to_string());

    // Test Half Floor
    let mut dec = Decimal::from_str("3.234").unwrap();
    dec.round_with_mode(0, SignedRoundingMode::HalfFloor);
    assert_eq!("3", dec.to_string());

    let mut dec = Decimal::from_str("3.534").unwrap();
    dec.round_with_mode(0, SignedRoundingMode::HalfFloor);
    assert_eq!("4", dec.to_string());

    let mut dec = Decimal::from_str("3.934").unwrap();
    dec.round_with_mode(0, SignedRoundingMode::HalfFloor);
    assert_eq!("4", dec.to_string());

    let mut dec = Decimal::from_str("2.222").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfFloor);
    assert_eq!("2.2", dec.to_string());

    let mut dec = Decimal::from_str("2.44").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfFloor);
    assert_eq!("2.4", dec.to_string());

    let mut dec = Decimal::from_str("2.45").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfFloor);
    assert_eq!("2.4", dec.to_string());

    let mut dec = Decimal::from_str("-2.44").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfFloor);
    assert_eq!("-2.4", dec.to_string());

    let mut dec = Decimal::from_str("-2.45").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfFloor);
    assert_eq!("-2.5", dec.to_string());

    let mut dec = Decimal::from_str("22.222").unwrap();
    dec.round_with_mode(-2, SignedRoundingMode::HalfFloor);
    assert_eq!("22.22", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.round_with_mode(-2, SignedRoundingMode::HalfFloor);
    assert_eq!("100.00", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.round_with_mode(-5, SignedRoundingMode::HalfFloor);
    assert_eq!("99.99900", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode(-5, SignedRoundingMode::HalfFloor);
    assert_eq!("-99.99900", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode(-2, SignedRoundingMode::HalfFloor);
    assert_eq!("-100.00", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.round_with_mode(4, SignedRoundingMode::HalfFloor);
    assert_eq!("0000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode(4, SignedRoundingMode::HalfFloor);
    assert_eq!("-0000", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfFloor);
    assert_eq!("0.0", dec.to_string());

    let mut dec = Decimal::from_str("-0.009").unwrap();
    dec.round_with_mode(-1, SignedRoundingMode::HalfFloor);
    assert_eq!("-0.0", dec.to_string());

    // Test Truncate Right
    let mut dec = Decimal::from(4235970u32);
    dec.multiply_pow10(-3);
    assert_eq!("4235.970", dec.to_string());

    dec.trunc(-5);
    assert_eq!("4235.97000", dec.to_string());

    dec.trunc(-1);
    assert_eq!("4235.9", dec.to_string());

    dec.trunc(0);
    assert_eq!("4235", dec.to_string());

    dec.trunc(1);
    assert_eq!("4230", dec.to_string());

    dec.trunc(5);
    assert_eq!("00000", dec.to_string());

    dec.trunc(2);
    assert_eq!("00000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.trunc(-2);
    assert_eq!("-99.99", dec.to_string());

    let mut dec = Decimal::from_str("1234.56").unwrap();
    dec.trunc(-1);
    assert_eq!("1234.5", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.trunc(-1);
    assert_eq!("0.0", dec.to_string());

    // Test trunced
    let mut dec = Decimal::from(4235970u32);
    dec.multiply_pow10(-3);
    assert_eq!("4235.970", dec.to_string());

    assert_eq!("4235.97000", dec.clone().trunced(-5).to_string());

    assert_eq!("4230", dec.clone().trunced(1).to_string());

    assert_eq!("4200", dec.clone().trunced(2).to_string());

    assert_eq!("00000", dec.trunced(5).to_string());

    //Test expand
    let mut dec = Decimal::from_str("3.234").unwrap();
    dec.expand(0);
    assert_eq!("4", dec.to_string());

    let mut dec = Decimal::from_str("2.222").unwrap();
    dec.expand(-1);
    assert_eq!("2.3", dec.to_string());

    let mut dec = Decimal::from_str("22.222").unwrap();
    dec.expand(-2);
    assert_eq!("22.23", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.expand(-2);
    assert_eq!("100.00", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.expand(-5);
    assert_eq!("99.99900", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.expand(-5);
    assert_eq!("-99.99900", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.expand(-2);
    assert_eq!("-100.00", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.expand(4);
    assert_eq!("10000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.expand(4);
    assert_eq!("-10000", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.expand(-1);
    assert_eq!("0.1", dec.to_string());

    let mut dec = Decimal::from_str("-0.009").unwrap();
    dec.expand(-1);
    assert_eq!("-0.1", dec.to_string());

    let mut dec = Decimal::from_str("3.954").unwrap();
    dec.expand(0);
    assert_eq!("4", dec.to_string());

    // Test half_expand
    let mut dec = Decimal::from_str("3.234").unwrap();
    dec.round_with_mode(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("3", dec.to_string());

    let mut dec = Decimal::from_str("3.534").unwrap();
    dec.round_with_mode(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("4", dec.to_string());

    let mut dec = Decimal::from_str("3.934").unwrap();
    dec.round_with_mode(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("4", dec.to_string());

    let mut dec = Decimal::from_str("2.222").unwrap();
    dec.round_with_mode(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("2.2", dec.to_string());

    let mut dec = Decimal::from_str("2.44").unwrap();
    dec.round_with_mode(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("2.4", dec.to_string());

    let mut dec = Decimal::from_str("2.45").unwrap();
    dec.round_with_mode(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("2.5", dec.to_string());

    let mut dec = Decimal::from_str("-2.44").unwrap();
    dec.round_with_mode(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("-2.4", dec.to_string());

    let mut dec = Decimal::from_str("-2.45").unwrap();
    dec.round_with_mode(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("-2.5", dec.to_string());

    let mut dec = Decimal::from_str("22.222").unwrap();
    dec.round_with_mode(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("22.22", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.round_with_mode(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("100.00", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.round_with_mode(
        -5,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("99.99900", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode(
        -5,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("-99.99900", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("-100.00", dec.to_string());

    let mut dec = Decimal::from_str("99.999").unwrap();
    dec.round_with_mode(
        4,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("0000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode(
        4,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("-0000", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("0.0", dec.to_string());

    let mut dec = Decimal::from_str("-0.009").unwrap();
    dec.round_with_mode(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
    );
    assert_eq!("-0.0", dec.to_string());

    // Test specific cases
    let mut dec = Decimal::from_str("1.108").unwrap();
    dec.round_with_mode(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
    );
    assert_eq!("1.11", dec.to_string());

    let mut dec = Decimal::from_str("1.108").unwrap();
    dec.expand(-2);
    assert_eq!("1.11", dec.to_string());

    let mut dec = Decimal::from_str("1.108").unwrap();
    dec.trunc(-2);
    assert_eq!("1.10", dec.to_string());

    let mut dec = Decimal::from_str("2.78536913177").unwrap();
    dec.round_with_mode(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
    );
    assert_eq!("2.79", dec.to_string());
}

#[test]
fn test_concatenate() {
    #[derive(Debug)]
    struct TestCase {
        pub input_1: &'static str,
        pub input_2: &'static str,
        pub expected: Option<&'static str>,
    }
    let cases = [
        TestCase {
            input_1: "123",
            input_2: "0.456",
            expected: Some("123.456"),
        },
        TestCase {
            input_1: "0.456",
            input_2: "123",
            expected: None,
        },
        TestCase {
            input_1: "123",
            input_2: "0.0456",
            expected: Some("123.0456"),
        },
        TestCase {
            input_1: "0.0456",
            input_2: "123",
            expected: None,
        },
        TestCase {
            input_1: "100",
            input_2: "0.456",
            expected: Some("100.456"),
        },
        TestCase {
            input_1: "0.456",
            input_2: "100",
            expected: None,
        },
        TestCase {
            input_1: "100",
            input_2: "0.001",
            expected: Some("100.001"),
        },
        TestCase {
            input_1: "0.001",
            input_2: "100",
            expected: None,
        },
        TestCase {
            input_1: "123000",
            input_2: "456",
            expected: Some("123456"),
        },
        TestCase {
            input_1: "456",
            input_2: "123000",
            expected: None,
        },
        TestCase {
            input_1: "5",
            input_2: "5",
            expected: None,
        },
        TestCase {
            input_1: "120",
            input_2: "25",
            expected: None,
        },
        TestCase {
            input_1: "1.1",
            input_2: "0.2",
            expected: None,
        },
        TestCase {
            input_1: "0",
            input_2: "222",
            expected: Some("222"),
        },
        TestCase {
            input_1: "222",
            input_2: "0",
            expected: Some("222"),
        },
        TestCase {
            input_1: "0",
            input_2: "0",
            expected: Some("0"),
        },
        TestCase {
            input_1: "000",
            input_2: "0",
            expected: Some("000"),
        },
        TestCase {
            input_1: "0.00",
            input_2: "0",
            expected: Some("0.00"),
        },
    ];
    for cas in &cases {
        let fd1 = Decimal::from_str(cas.input_1).unwrap();
        let fd2 = Decimal::from_str(cas.input_2).unwrap();
        match fd1.absolute.concatenated_end(fd2.absolute) {
            Ok(fd) => {
                assert_eq!(cas.expected, Some(fd.to_string().as_str()), "{cas:?}");
            }
            Err(_) => {
                assert!(cas.expected.is_none(), "{cas:?}");
            }
        }
    }
}

#[test]
fn test_rounding_increment() {
    // Test Truncate Right
    let mut dec = Decimal::from(4235970u32);
    dec.multiply_pow10(-3);
    assert_eq!("4235.970", dec.to_string());

    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("4235.96", dec.to_string());

    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("4235.5", dec.to_string());

    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("4225", dec.to_string());

    dec.round_with_mode_and_increment(
        5,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("00000", dec.to_string());

    dec.round_with_mode_and_increment(
        2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("00000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("-99.75", dec.to_string());

    let mut dec = Decimal::from_str("1234.56").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("1234.4", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("0.0", dec.to_string());

    let mut dec = Decimal::from_str("0.60").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.40").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.25", dec.to_string());

    let mut dec = Decimal::from_str("0.7000000099").unwrap();
    dec.round_with_mode_and_increment(
        -3,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("0.700", dec.to_string());

    let mut dec = Decimal::from_str("5").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0", dec.to_string());

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf2,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(6u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(9u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf5,
    );

    let expected_min = {
        let mut expected_min = Decimal::from(5u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };

    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(70u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf25,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(50u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf2,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(6u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf5,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(5u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Trunc),
        RoundingIncrement::MultiplesOf25,
    );
    let expected = {
        let mut expected = Decimal::from(0u32);
        expected.multiply_pow10(i16::MAX);
        expected
    };
    assert_eq!(expected, dec);

    // Test Expand
    let mut dec = Decimal::from(4235970u32);
    dec.multiply_pow10(-3);
    assert_eq!("4235.970", dec.to_string());

    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("4235.98", dec.to_string());

    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("4236.0", dec.to_string());

    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("4250", dec.to_string());

    dec.round_with_mode_and_increment(
        5,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("500000", dec.to_string());

    dec.round_with_mode_and_increment(
        2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("500000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("-100.00", dec.to_string());

    let mut dec = Decimal::from_str("1234.56").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("1234.6", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("0.5", dec.to_string());

    let mut dec = Decimal::from_str("0.60").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.75", dec.to_string());

    let mut dec = Decimal::from_str("0.40").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.7000000099").unwrap();
    dec.round_with_mode_and_increment(
        -3,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("0.702", dec.to_string());

    let mut dec = Decimal::from_str("5").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("25", dec.to_string());

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(8u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(9u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(10u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(70u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(75u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(8u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(0u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(0u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    // Test Half Truncate Right
    let mut dec = Decimal::from(4235970u32);
    dec.multiply_pow10(-3);
    assert_eq!("4235.970", dec.to_string());

    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("4235.96", dec.to_string());

    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("4236.0", dec.to_string());

    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("4225", dec.to_string());

    dec.round_with_mode_and_increment(
        5,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("00000", dec.to_string());

    dec.round_with_mode_and_increment(
        2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("00000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("-100.00", dec.to_string());

    let mut dec = Decimal::from_str("1234.56").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("1234.6", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("0.0", dec.to_string());

    let mut dec = Decimal::from_str("0.60").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.40").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.7000000099").unwrap();
    dec.round_with_mode_and_increment(
        -3,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("0.700", dec.to_string());

    let mut dec = Decimal::from_str("5").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0", dec.to_string());

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf2,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(6u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(9u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf5,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(10u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(70u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf25,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(75u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf2,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(6u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf5,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(5u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf25,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(0u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    // Test Half Expand
    let mut dec = Decimal::from(4235970u32);
    dec.multiply_pow10(-3);
    assert_eq!("4235.970", dec.to_string());

    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("4235.98", dec.to_string());

    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("4236.0", dec.to_string());

    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("4225", dec.to_string());

    dec.round_with_mode_and_increment(
        5,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("00000", dec.to_string());

    dec.round_with_mode_and_increment(
        2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("00000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("-100.00", dec.to_string());

    let mut dec = Decimal::from_str("1234.56").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("1234.6", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("0.0", dec.to_string());

    let mut dec = Decimal::from_str("0.60").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.40").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.7000000099").unwrap();
    dec.round_with_mode_and_increment(
        -3,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("0.700", dec.to_string());

    let mut dec = Decimal::from_str("5").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0", dec.to_string());

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf2,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(8u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(9u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf5,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(10u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(70u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf25,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(75u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf2,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(8u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    // Test Ceil
    let mut dec = Decimal::from(4235970u32);
    dec.multiply_pow10(-3);
    assert_eq!("4235.970", dec.to_string());

    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("4235.98", dec.to_string());

    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("4236.0", dec.to_string());

    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("4250", dec.to_string());

    dec.round_with_mode_and_increment(5, SignedRoundingMode::Ceil, RoundingIncrement::MultiplesOf5);
    assert_eq!("500000", dec.to_string());

    dec.round_with_mode_and_increment(2, SignedRoundingMode::Ceil, RoundingIncrement::MultiplesOf2);
    assert_eq!("500000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("-99.75", dec.to_string());

    let mut dec = Decimal::from_str("1234.56").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("1234.6", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("0.5", dec.to_string());

    let mut dec = Decimal::from_str("0.60").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.75", dec.to_string());

    let mut dec = Decimal::from_str("0.40").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.7000000099").unwrap();
    dec.round_with_mode_and_increment(
        -3,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("0.702", dec.to_string());

    let mut dec = Decimal::from_str("5").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("25", dec.to_string());

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf2,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(8u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(9u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf5,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(10u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(70u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf25,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(75u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Ceil,
        RoundingIncrement::MultiplesOf2,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(8u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    // Test Half Ceil
    let mut dec = Decimal::from(4235970u32);
    dec.multiply_pow10(-3);
    assert_eq!("4235.970", dec.to_string());

    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("4235.98", dec.to_string());

    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("4236.0", dec.to_string());

    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("4225", dec.to_string());

    dec.round_with_mode_and_increment(
        5,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("00000", dec.to_string());

    dec.round_with_mode_and_increment(
        2,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("00000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("-100.00", dec.to_string());

    let mut dec = Decimal::from_str("1234.56").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("1234.6", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("0.0", dec.to_string());

    let mut dec = Decimal::from_str("0.60").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.40").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.7000000099").unwrap();
    dec.round_with_mode_and_increment(
        -3,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("0.700", dec.to_string());

    let mut dec = Decimal::from_str("5").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0", dec.to_string());

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf2,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(8u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(9u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf5,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(10u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(70u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf25,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(75u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf2,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(8u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    // Test Floor
    let mut dec = Decimal::from(4235970u32);
    dec.multiply_pow10(-3);
    assert_eq!("4235.970", dec.to_string());

    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("4235.96", dec.to_string());

    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("4235.5", dec.to_string());

    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("4225", dec.to_string());

    dec.round_with_mode_and_increment(
        5,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("00000", dec.to_string());

    dec.round_with_mode_and_increment(
        2,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("00000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("-100.00", dec.to_string());

    let mut dec = Decimal::from_str("1234.56").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("1234.4", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("0.0", dec.to_string());

    let mut dec = Decimal::from_str("0.60").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.40").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.25", dec.to_string());

    let mut dec = Decimal::from_str("0.7000000099").unwrap();
    dec.round_with_mode_and_increment(
        -3,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("0.700", dec.to_string());

    let mut dec = Decimal::from_str("5").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0", dec.to_string());

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf2,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(6u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(9u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf5,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(5u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(70u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf25,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(50u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Floor,
        RoundingIncrement::MultiplesOf2,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(6u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    // Test Half Floor
    let mut dec = Decimal::from(4235970u32);
    dec.multiply_pow10(-3);
    assert_eq!("4235.970", dec.to_string());

    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("4235.96", dec.to_string());

    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("4236.0", dec.to_string());

    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("4225", dec.to_string());

    dec.round_with_mode_and_increment(
        5,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("00000", dec.to_string());

    dec.round_with_mode_and_increment(
        2,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("00000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("-100.00", dec.to_string());

    let mut dec = Decimal::from_str("1234.56").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("1234.6", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("0.0", dec.to_string());

    let mut dec = Decimal::from_str("0.60").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.40").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.7000000099").unwrap();
    dec.round_with_mode_and_increment(
        -3,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("0.700", dec.to_string());

    let mut dec = Decimal::from_str("5").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0", dec.to_string());

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf2,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(6u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(9u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf5,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(10u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(70u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf25,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(75u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf2,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(6u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    // Test Half Even
    let mut dec = Decimal::from(4235970u32);
    dec.multiply_pow10(-3);
    assert_eq!("4235.970", dec.to_string());

    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("4235.96", dec.to_string());

    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("4236.0", dec.to_string());

    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("4225", dec.to_string());

    dec.round_with_mode_and_increment(
        5,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("00000", dec.to_string());

    dec.round_with_mode_and_increment(
        2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("00000", dec.to_string());

    let mut dec = Decimal::from_str("-99.999").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("-100.00", dec.to_string());

    let mut dec = Decimal::from_str("1234.56").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("1234.6", dec.to_string());

    let mut dec = Decimal::from_str("0.009").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("0.0", dec.to_string());

    let mut dec = Decimal::from_str("0.60").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.40").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("0.7000000099").unwrap();
    dec.round_with_mode_and_increment(
        -3,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("0.700", dec.to_string());

    let mut dec = Decimal::from_str("5").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0", dec.to_string());

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(8u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(9u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf5,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(10u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(70u32);
    dec.multiply_pow10(i16::MIN);
    dec.round_with_mode_and_increment(
        i16::MIN,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf25,
    );
    let expected_min = {
        let mut expected_min = Decimal::from(75u32);
        expected_min.multiply_pow10(i16::MIN);
        expected_min
    };
    assert_eq!(expected_min, dec);

    let mut dec = Decimal::from(7u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(8u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    // Test specific cases
    let mut dec = Decimal::from_str("1.108").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("1.12", dec.to_string());

    let mut dec = Decimal::from_str("1.108").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("1.15", dec.to_string());

    let mut dec = Decimal::from_str("1.108").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("1.25", dec.to_string());

    let mut dec = Decimal::from(9u32);
    dec.multiply_pow10(i16::MAX - 1);
    dec.round_with_mode_and_increment(
        i16::MAX - 1,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    let expected_max_minus_1 = {
        let mut expected_max_minus_1 = Decimal::from(25u32);
        expected_max_minus_1.multiply_pow10(i16::MAX - 1);
        expected_max_minus_1
    };
    assert_eq!(expected_max_minus_1, dec);

    let mut dec = Decimal::from(9u32);
    dec.multiply_pow10(i16::MAX);
    dec.round_with_mode_and_increment(
        i16::MAX,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    let expected_max = {
        let mut expected_max = Decimal::from(0u32);
        expected_max.multiply_pow10(i16::MAX);
        expected_max
    };
    assert_eq!(expected_max, dec);

    let mut dec = Decimal::from_str("0").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("0", dec.to_string());

    let mut dec = Decimal::from_str("0").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("0", dec.to_string());

    let mut dec = Decimal::from_str("0").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0", dec.to_string());

    let mut dec = Decimal::from_str("0.1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2", dec.to_string());

    let mut dec = Decimal::from_str("0.1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("5", dec.to_string());

    let mut dec = Decimal::from_str("0.1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("25", dec.to_string());

    let mut dec = Decimal::from_str("1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2", dec.to_string());

    let mut dec = Decimal::from_str("1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("5", dec.to_string());

    let mut dec = Decimal::from_str("1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("25", dec.to_string());

    let mut dec = Decimal::from_str("2").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2", dec.to_string());

    let mut dec = Decimal::from_str("2").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("5", dec.to_string());

    let mut dec = Decimal::from_str("2.1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("4", dec.to_string());

    let mut dec = Decimal::from_str("2.1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("5", dec.to_string());

    let mut dec = Decimal::from_str("4").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("4", dec.to_string());

    let mut dec = Decimal::from_str("4").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("5", dec.to_string());

    let mut dec = Decimal::from_str("4.1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("6", dec.to_string());

    let mut dec = Decimal::from_str("4.1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("5", dec.to_string());

    let mut dec = Decimal::from_str("5").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("6", dec.to_string());

    let mut dec = Decimal::from_str("5").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("5", dec.to_string());

    let mut dec = Decimal::from_str("5.1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("6", dec.to_string());

    let mut dec = Decimal::from_str("5.1").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("10", dec.to_string());

    let mut dec = Decimal::from_str("6").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("6", dec.to_string());

    let mut dec = Decimal::from_str("6").unwrap();
    dec.round_with_mode_and_increment(
        0,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("10", dec.to_string());

    let mut dec = Decimal::from_str("0.50").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("0.50", dec.to_string());

    let mut dec = Decimal::from_str("1.1025").unwrap();
    dec.round_with_mode_and_increment(
        -3,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfTrunc),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("1.100", dec.to_string());

    let mut dec = Decimal::from_str("1.10125").unwrap();
    dec.round_with_mode_and_increment(
        -4,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfExpand),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("1.1025", dec.to_string());

    let mut dec = Decimal::from_str("-1.25").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("-1.0", dec.to_string());

    let mut dec = Decimal::from_str("-1.251").unwrap();
    dec.round_with_mode_and_increment(
        -1,
        SignedRoundingMode::HalfCeil,
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("-1.5", dec.to_string());

    let mut dec = Decimal::from_str("-1.125").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::HalfFloor,
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("-1.25", dec.to_string());

    let mut dec = Decimal::from_str("2.71").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2.72", dec.to_string());

    let mut dec = Decimal::from_str("2.73").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2.72", dec.to_string());

    let mut dec = Decimal::from_str("2.75").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2.76", dec.to_string());

    let mut dec = Decimal::from_str("2.77").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2.76", dec.to_string());

    let mut dec = Decimal::from_str("2.79").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2.80", dec.to_string());

    let mut dec = Decimal::from_str("2.41").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2.40", dec.to_string());

    let mut dec = Decimal::from_str("2.43").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2.44", dec.to_string());

    let mut dec = Decimal::from_str("2.45").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2.44", dec.to_string());

    let mut dec = Decimal::from_str("2.47").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2.48", dec.to_string());

    let mut dec = Decimal::from_str("2.49").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf2,
    );
    assert_eq!("2.48", dec.to_string());

    let mut dec = Decimal::from_str("2.725").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("2.70", dec.to_string());

    let mut dec = Decimal::from_str("2.775").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf5,
    );
    assert_eq!("2.80", dec.to_string());

    let mut dec = Decimal::from_str("2.875").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("3.00", dec.to_string());

    let mut dec = Decimal::from_str("2.375").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("2.50", dec.to_string());

    let mut dec = Decimal::from_str("2.125").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("2.00", dec.to_string());

    let mut dec = Decimal::from_str("2.625").unwrap();
    dec.round_with_mode_and_increment(
        -2,
        SignedRoundingMode::Unsigned(UnsignedRoundingMode::HalfEven),
        RoundingIncrement::MultiplesOf25,
    );
    assert_eq!("2.50", dec.to_string());
}
