// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::fmt;
use core::str::FromStr;

use crate::Decimal;
use crate::ParseError;

/// A struct containing a [`Decimal`] significand together with an exponent, representing a
/// number written in compact notation (such as 1.2M).
/// This represents a _source number_, as defined
/// [in UTS #35](https://www.unicode.org/reports/tr35/tr35-numbers.html#Plural_rules_syntax).
/// The value exponent=0 represents a number in non-compact
/// notation (such as 1 200 000).
///
/// This is distinct from [`crate::ScientificDecimal`] because it does not represent leading 0s
/// nor a sign in the exponent, and behaves differently in pluralization.
#[derive(Debug, Clone, PartialEq)]
pub struct CompactDecimal {
    significand: Decimal,
    exponent: u8,
}

impl CompactDecimal {
    /// Constructs a [`CompactDecimal`] from its significand and exponent.
    pub fn from_significand_and_exponent(significand: Decimal, exponent: u8) -> Self {
        Self {
            significand,
            exponent,
        }
    }

    /// Returns a reference to the significand of `self`.
    /// ```
    /// # use fixed_decimal::CompactDecimal;
    /// # use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    /// #
    /// assert_eq!(
    ///     CompactDecimal::from_str("+1.20c6").unwrap().significand(),
    ///     &Decimal::from_str("+1.20").unwrap()
    /// );
    /// ```
    pub fn significand(&self) -> &Decimal {
        &self.significand
    }

    /// Returns the significand of `self`.
    /// ```
    /// # use fixed_decimal::CompactDecimal;
    /// # use fixed_decimal::Decimal;
    /// # use std::str::FromStr;
    /// #
    /// assert_eq!(
    ///     CompactDecimal::from_str("+1.20c6")
    ///         .unwrap()
    ///         .into_significand(),
    ///     Decimal::from_str("+1.20").unwrap()
    /// );
    /// ```
    pub fn into_significand(self) -> Decimal {
        self.significand
    }

    /// Returns the exponent of `self`.
    /// ```
    /// # use fixed_decimal::CompactDecimal;
    /// # use std::str::FromStr;
    /// #
    /// assert_eq!(CompactDecimal::from_str("+1.20c6").unwrap().exponent(), 6);
    /// assert_eq!(CompactDecimal::from_str("1729").unwrap().exponent(), 0);
    /// ```
    pub fn exponent(&self) -> u8 {
        self.exponent
    }
}

/// Render the [`CompactDecimal`] in sampleValue syntax.
/// The letter c is used, rather than the deprecated e.
///
/// # Examples
///
/// ```
/// # use fixed_decimal::CompactDecimal;
/// # use std::str::FromStr;
/// # use writeable::assert_writeable_eq;
/// #
/// assert_writeable_eq!(
///     CompactDecimal::from_str("+1.20c6").unwrap(),
///     "+1.20c6"
/// );
/// assert_writeable_eq!(CompactDecimal::from_str("+1729").unwrap(), "+1729");
/// ```
impl writeable::Writeable for CompactDecimal {
    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
        self.significand.write_to(sink)?;
        if self.exponent != 0 {
            sink.write_char('c')?;
            self.exponent.write_to(sink)?;
        }
        Ok(())
    }

    fn writeable_length_hint(&self) -> writeable::LengthHint {
        let mut result = self.significand.writeable_length_hint();
        if self.exponent != 0 {
            result += self.exponent.writeable_length_hint() + 1;
        }
        result
    }
}

writeable::impl_display_with_writeable!(CompactDecimal);

impl CompactDecimal {
    #[inline]
    /// Parses a [`CompactDecimal`].
    pub fn try_from_str(s: &str) -> Result<Self, ParseError> {
        Self::try_from_utf8(s.as_bytes())
    }

    /// The deprecated letter e is not accepted as a synonym for c.
    fn try_from_utf8(code_units: &[u8]) -> Result<Self, ParseError> {
        if code_units.iter().any(|&c| c == b'e' || c == b'E') {
            return Err(ParseError::Syntax);
        }
        let mut parts = code_units.split(|&c| c == b'c');
        let significand = Decimal::try_from_utf8(parts.next().ok_or(ParseError::Syntax)?)?;
        match parts.next() {
            None => Ok(CompactDecimal {
                significand,
                exponent: 0,
            }),
            Some(exponent_str) => {
                let exponent_str =
                    core::str::from_utf8(exponent_str).map_err(|_| ParseError::Syntax)?;
                if parts.next().is_some() {
                    return Err(ParseError::Syntax);
                }
                if exponent_str.is_empty()
                    || exponent_str.bytes().next() == Some(b'0')
                    || !exponent_str.bytes().all(|c| c.is_ascii_digit())
                {
                    return Err(ParseError::Syntax);
                }
                let exponent = exponent_str.parse().map_err(|_| ParseError::Limit)?;
                Ok(CompactDecimal {
                    significand,
                    exponent,
                })
            }
        }
    }
}

impl FromStr for CompactDecimal {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_str(s)
    }
}

#[test]
fn test_compact_syntax_error() {
    #[derive(Debug)]
    struct TestCase {
        pub input_str: &'static str,
        pub expected_err: Option<ParseError>,
    }
    let cases = [
        TestCase {
            input_str: "-123e4",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "-123c",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "1c10",
            expected_err: None,
        },
        TestCase {
            input_str: "1E1c1",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "1e1c1",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "1c1e1",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "1c1E1",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "-1c01",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "-1c-1",
            expected_err: Some(ParseError::Syntax),
        },
        TestCase {
            input_str: "-1c1",
            expected_err: None,
        },
    ];
    for cas in &cases {
        match CompactDecimal::from_str(cas.input_str) {
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
