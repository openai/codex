// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This module defines variations of numeric types, including signed values,
//! values with infinity, and values with NaN.

// TODO: move to sign.rs
/// A specification of the sign used when formatting a number.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
#[allow(clippy::exhaustive_enums)]
// There are only 3 sign values, and they correspond to the low-level data model of FixedDecimal and UTS 35.
pub enum Sign {
    /// No sign (implicitly positive, e.g., 1729).
    #[default]
    None,
    /// A negative sign, e.g., -1729.
    Negative,
    /// An explicit positive sign, e.g., +1729.
    Positive,
}

/// Configuration for when to render the minus sign or plus sign.
#[non_exhaustive]
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum SignDisplay {
    /// Render the sign according to locale preferences. In most cases, this means a minus sign
    /// will be shown on negative numbers, and no sign will be shown on positive numbers.
    Auto,

    /// Do not display the sign. Positive and negative numbers are indistinguishable.
    Never,

    /// Show a minus sign on negative numbers and a plus sign on positive numbers, including zero.
    Always,

    /// Show a minus sign on negative numbers and a plus sign on positive numbers, except do not
    /// show any sign on positive or negative zero.
    ExceptZero,

    /// Show a minus sign on strictly negative numbers. Do not show a sign on positive numbers or
    /// on positive or negative zero.
    ///
    /// This differs from [`Auto`](SignDisplay::Auto) in that it does not render a sign on negative zero.
    Negative,
}

/// The `Signed` struct represents a numeric value with an associated sign.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Signed<T> {
    pub sign: Sign,
    pub absolute: T,
}

impl<T> Signed<T> {
    /// Returns the sign of this signed number.
    pub fn sign(&self) -> Sign {
        self.sign
    }

    /// Changes the sign of this signed number to the one given.
    pub fn set_sign(&mut self, sign: Sign) {
        self.sign = sign;
    }

    /// Returns this number with the sign changed to the one given.
    pub fn with_sign(mut self, sign: Sign) -> Self {
        self.set_sign(sign);
        self
    }
}

// TODO(#5065): implement `WithCompactExponent` and `WithScientificExponent`.
// /// The `WithInfinity` enum represents a numeric value that may be either infinite or finite.
// #[derive(Debug)]
// pub enum WithInfinity<T> {
//     Infinity,
//     Finite(T),
// }

// /// The `WithNaN` enum represents a numeric value that may be NaN.
// #[derive(Debug)]
// pub enum WithNaN<T> {
//     NaN,
//     N(T),
// }

// TODO(#5065): implement `WithCompactExponent` and `WithScientificExponent`.
// /// The `WithCompactExponent` struct represents a numeric value with a compact exponent.
// #[derive(Debug)]
// pub struct WithCompactExponent<T> {
//     pub exponent: u8,
//     pub significand: T,
// }

// /// The `WithScientificExponent` struct represents a numeric value with a scientific exponent.
// #[derive(Debug)]
// pub struct WithScientificExponent<T> {
//     pub exponent: i16,
//     pub significand: T,
// }
