// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::fmt;
use core::str::FromStr;

use crate::Decimal;
use crate::FixedInteger;
use crate::ParseError;

/// A struct containing a [`Decimal`] significand together with an exponent, representing a
/// number written in scientific notation, such as 1.729×10³.
///
/// This structure represents any 0s shown in the significand and exponent,
/// and an optional sign for both the significand and the exponent.
#[derive(Debug, Clone, PartialEq)]
pub struct ScientificDecimal {
    significand: Decimal,
    exponent: FixedInteger,
}

impl ScientificDecimal {
    pub fn from(significand: Decimal, exponent: FixedInteger) -> Self {
        ScientificDecimal {
            significand,
            exponent,
        }
    }
}

/// Render the [`ScientificDecimal`] as a string of ASCII digits with a possible decimal point,
/// followed by the letter 'e', and the exponent.
///
/// # Examples
///
/// ```
/// # use fixed_decimal::Decimal;
/// # use fixed_decimal::FixedInteger;
/// # use fixed_decimal::ScientificDecimal;
/// # use std::str::FromStr;
/// # use writeable::assert_writeable_eq;
/// #
/// assert_writeable_eq!(
///     ScientificDecimal::from(
///         {
///             let mut dec = Decimal::from(1729u32);
///             dec.multiply_pow10(-3);
///             dec
///         },
///         FixedInteger::from(3)
///     ),
///     "1.729e3"
/// );
/// assert_writeable_eq!(
///     ScientificDecimal::from(
///         Decimal::from_str("+1.729").unwrap(),
///         FixedInteger::from_str("+03").unwrap()
///     ),
///     "+1.729e+03"
/// );
/// ```
impl writeable::Writeable for ScientificDecimal {
    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        self.significand.write_to(sink)?;
        sink.write_char('e')?;
        self.exponent.write_to(sink)
    }

    fn writeable_length_hint(&self) -> writeable::LengthHint {
        self.significand.writeable_length_hint() + 1 + self.exponent.writeable_length_hint()
    }
}

writeable::impl_display_with_writeable!(ScientificDecimal);

impl ScientificDecimal {
    #[inline]
    pub fn try_from_str(s: &str) -> Result<Self, ParseError> {
        Self::try_from_utf8(s.as_bytes())
    }

    pub fn try_from_utf8(code_units: &[u8]) -> Result<Self, ParseError> {
        // Fixed_Decimal::try_from supports scientific notation; ensure that
        // we don’t accept something like 1e1E1.  Splitting on 'e' ensures that
        // we disallow 1e1e1.
        if code_units.contains(&b'E') {
            return Err(ParseError::Syntax);
        }
        let mut parts = code_units.split(|&c| c == b'e');
        let significand = parts.next().ok_or(ParseError::Syntax)?;
        let exponent = parts.next().ok_or(ParseError::Syntax)?;
        if parts.next().is_some() {
            return Err(ParseError::Syntax);
        }
        Ok(ScientificDecimal::from(
            Decimal::try_from_utf8(significand)?,
            FixedInteger::try_from_utf8(exponent)?,
        ))
    }
}

impl FromStr for ScientificDecimal {
    type Err = ParseError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_str(s)
    }
}

#[test]
fn test_scientific_syntax_error() {
    #[derive(Debug)]
    struct TestCase {
        pub input_str: &'static str,
        pub expected_err: Option<ParseError>,
    }
    let cases = [
        TestCase {
            input_str: "5",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "-123c4",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "-123e",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "1e10",
            expected_err: None,
        },
        TestCase {
            input_str: "1e1e1",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "1e1E1",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "1E1e1",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "-1e+01",
            expected_err: None,
        },
        TestCase {
            input_str: "-1e+1.0",
            expected_err: Some(ParseError::Limit),
        },
        TestCase {
            input_str: "-1e+-1",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "123E4",
            expected_err: Some(ParseError::Syntax),
        },
    ];
    for cas in &cases {
        match ScientificDecimal::from_str(cas.input_str) {
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
