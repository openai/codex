// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This file contains the implementation of the rounding algorithm and the related functions & types.

// Adapters to convert runtime dispatched calls into const-inlined methods.
// This allows reducing the codesize for the common case of no increment.

#[derive(Copy, Clone, PartialEq)]
pub(crate) struct NoIncrement;

pub(crate) trait IncrementLike: Copy + Sized + PartialEq {
    const MULTIPLES_OF_1: Option<Self>;
    const MULTIPLES_OF_2: Option<Self>;
    const MULTIPLES_OF_5: Option<Self>;
    const MULTIPLES_OF_25: Option<Self>;
}

impl IncrementLike for RoundingIncrement {
    const MULTIPLES_OF_1: Option<Self> = Some(Self::MultiplesOf1);

    const MULTIPLES_OF_2: Option<Self> = Some(Self::MultiplesOf2);

    const MULTIPLES_OF_5: Option<Self> = Some(Self::MultiplesOf5);

    const MULTIPLES_OF_25: Option<Self> = Some(Self::MultiplesOf25);
}

impl IncrementLike for NoIncrement {
    const MULTIPLES_OF_1: Option<Self> = Some(Self);

    const MULTIPLES_OF_2: Option<Self> = None;

    const MULTIPLES_OF_5: Option<Self> = None;

    const MULTIPLES_OF_25: Option<Self> = None;
}

/// Mode used in a unsigned rounding operations.
///
/// # Comparative table of all the rounding modes, including the signed and unsigned ones.
///
/// | Value | Ceil | Expand | Floor | Trunc | HalfCeil | HalfExpand | HalfFloor | HalfTrunc | HalfEven |
/// |:-----:|:----:|:------:|:-----:|:-----:|:--------:|:----------:|:---------:|:---------:|:--------:|
/// |  +1.8 |  +2  |   +2   |   +1  |   +1  |    +2    |     +2     |     +2    |     +2    |    +2    |
/// |  +1.5 |   "  |    "   |   "   |   "   |     "    |      "     |     +1    |     +1    |     "    |
/// |  +1.2 |   "  |    "   |   "   |   "   |    +1    |     +1     |     "     |     "     |    +1    |
/// |  +0.8 |  +1  |   +1   |   0   |   0   |     "    |      "     |     "     |     "     |     "    |
/// |  +0.5 |   "  |    "   |   "   |   "   |     "    |      "     |     0     |     0     |     0    |
/// |  +0.2 |   "  |    "   |   "   |   "   |     0    |      0     |     "     |     "     |     "    |
/// |  -0.2 |   0  |   -1   |   -1  |   "   |     "    |      "     |     "     |     "     |     "    |
/// |  -0.5 |   "  |    "   |   "   |   "   |     "    |     -1     |     -1    |     "     |     "    |
/// |  -0.8 |   "  |    "   |   "   |   "   |    -1    |      "     |     "     |     -1    |    -1    |
/// |  -1.2 |  -1  |   -2   |   -2  |   -1  |     "    |      "     |     "     |     "     |     "    |
/// |  -1.5 |   "  |    "   |   "   |   "   |     "    |     -2     |     -2    |     "     |    -2    |
/// |  -1.8 |   "  |    "   |   "   |   "   |    -2    |      "     |     "     |     -2    |     "    |
///
/// NOTE:
///   - Ceil, Floor, HalfCeil and HalfFloor are part of the [`SignedRoundingMode`] enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum UnsignedRoundingMode {
    Expand,
    Trunc,
    HalfExpand,
    HalfTrunc,
    HalfEven,
}

/// Mode used in a signed rounding operations.
///
/// NOTE:
///   - You can find the comparative table of all the rounding modes in the [`UnsignedRoundingMode`] documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum SignedRoundingMode {
    Unsigned(UnsignedRoundingMode),
    Ceil,
    Floor,
    HalfCeil,
    HalfFloor,
}

/// Increment used in a rounding operation.
///
/// Forces a rounding operation to round to only multiples of the specified increment.
///
/// # Example
///
/// ```
/// use fixed_decimal::{Decimal, RoundingIncrement, SignedRoundingMode, UnsignedRoundingMode};
/// use writeable::assert_writeable_eq;
/// # use std::str::FromStr;
/// let dec = Decimal::from_str("-7.266").unwrap();
/// let mode = SignedRoundingMode::Unsigned(UnsignedRoundingMode::Expand);
/// let increments = [
///     // .266 normally expands to .27 when rounding on position -2...
///     (RoundingIncrement::MultiplesOf1, "-7.27"),
///     // ...however, when rounding to multiples of 2, .266 expands to .28, since the next multiple
///     // of 2 bigger than the least significant digit of the rounded value (7) is 8.
///     (RoundingIncrement::MultiplesOf2, "-7.28"),
///     // .266 expands to .30, since the next multiple of 5 bigger than 7 is 10.
///     (RoundingIncrement::MultiplesOf5, "-7.30"),
///     // .266 expands to .50, since the next multiple of 25 bigger than 27 is 50.
///     // Note how we compare against 27 instead of only 7, because the increment applies to
///     // the two least significant digits of the rounded value instead of only the least
///     // significant digit.
///     (RoundingIncrement::MultiplesOf25, "-7.50"),
/// ];
///
/// for (increment, expected) in increments {
///     assert_writeable_eq!(
///         dec.clone().rounded_with_mode_and_increment(
///             -2,
///             mode,
///             increment
///         ),
///         expected
///     );
/// }
/// ```
#[derive(Debug, Eq, PartialEq, Clone, Copy, Default)]
#[non_exhaustive]
pub enum RoundingIncrement {
    /// Round the least significant digit to any digit (0-9).
    ///
    /// This is the default rounding increment for all the methods that don't take a
    /// `RoundingIncrement` as an argument.
    #[default]
    MultiplesOf1,
    /// Round the least significant digit to multiples of two (0, 2, 4, 6, 8).
    MultiplesOf2,
    /// Round the least significant digit to multiples of five (0, 5).
    MultiplesOf5,
    /// Round the two least significant digits to multiples of twenty-five (0, 25, 50, 75).
    ///
    /// With this increment, the rounding position index will match the least significant digit
    /// of the multiple of 25; e.g. the number .264 expanded at position -2 using increments of 25
    /// will give .50 as a result, since the next multiple of 25 bigger than 26 is 50.
    MultiplesOf25,
}

/// Specifies the precision of a floating point value when constructing a FixedDecimal.
///
/// IEEE 754 is a representation of a point on the number line. On the other hand, FixedDecimal
/// specifies not only the point on the number line but also the precision of the number to a
/// specific power of 10. This enum augments a floating-point value with the additional
/// information required by FixedDecimal.
#[non_exhaustive]
#[cfg(feature = "ryu")]
#[derive(Debug, Clone, Copy)]
pub enum FloatPrecision {
    /// Specify that the floating point number is integer-valued.
    ///
    /// If the floating point is not actually integer-valued, an error will be returned.
    Integer,

    /// Specify that the floating point number is precise to a specific power of 10.
    /// The number may be rounded or trailing zeros may be added as necessary.
    Magnitude(i16),

    /// Specify that the floating point number is precise to a specific number of significant digits.
    /// The number may be rounded or trailing zeros may be added as necessary.
    ///
    /// The number requested may not be zero
    SignificantDigits(u8),

    /// Specify that the floating point number is precise to the maximum representable by IEEE.
    ///
    /// This results in a FixedDecimal having enough digits to recover the original floating point
    /// value, with no trailing zeros.
    RoundTrip,
}
