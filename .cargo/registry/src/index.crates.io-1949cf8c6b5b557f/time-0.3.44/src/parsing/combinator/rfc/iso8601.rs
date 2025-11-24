//! Rules defined in [ISO 8601].
//!
//! [ISO 8601]: https://www.iso.org/iso-8601-date-and-time-format.html

use core::num::NonZero;

#[allow(unused_imports, reason = "MSRV of 1.87")]
use num_conv::prelude::*;

use crate::parsing::combinator::{any_digit, ascii_char, exactly_n_digits, first_match, sign};
use crate::parsing::ParsedItem;
use crate::{Month, Weekday};

/// What kind of format is being parsed. This is used to ensure each part of the format (date, time,
/// offset) is the same kind.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ExtendedKind {
    /// The basic format.
    Basic,
    /// The extended format.
    Extended,
    /// ¯\_(ツ)_/¯
    Unknown,
}

impl ExtendedKind {
    /// Is it possible that the format is extended?
    #[inline]
    pub(crate) const fn maybe_extended(self) -> bool {
        matches!(self, Self::Extended | Self::Unknown)
    }

    /// Is the format known for certain to be extended?
    #[inline]
    pub(crate) const fn is_extended(self) -> bool {
        matches!(self, Self::Extended)
    }

    /// If the kind is `Unknown`, make it `Basic`. Otherwise, do nothing. Returns `Some` if and only
    /// if the kind is now `Basic`.
    #[inline]
    pub(crate) fn coerce_basic(&mut self) -> Option<()> {
        match self {
            Self::Basic => Some(()),
            Self::Extended => None,
            Self::Unknown => {
                *self = Self::Basic;
                Some(())
            }
        }
    }

    /// If the kind is `Unknown`, make it `Extended`. Otherwise, do nothing. Returns `Some` if and
    /// only if the kind is now `Extended`.
    #[inline]
    pub(crate) fn coerce_extended(&mut self) -> Option<()> {
        match self {
            Self::Basic => None,
            Self::Extended => Some(()),
            Self::Unknown => {
                *self = Self::Extended;
                Some(())
            }
        }
    }
}

/// Parse a possibly expanded year.
#[inline]
pub(crate) fn year(input: &[u8]) -> Option<ParsedItem<'_, i32>> {
    Some(match sign(input) {
        Some(ParsedItem(input, sign)) => exactly_n_digits::<6, u32>(input)?.map(|val| {
            let val = val.cast_signed();
            if sign == b'-' {
                -val
            } else {
                val
            }
        }),
        None => exactly_n_digits::<4, u32>(input)?.map(|val| val.cast_signed()),
    })
}

/// Parse a month.
#[inline]
pub(crate) fn month(input: &[u8]) -> Option<ParsedItem<'_, Month>> {
    first_match(
        [
            (b"01".as_slice(), Month::January),
            (b"02".as_slice(), Month::February),
            (b"03".as_slice(), Month::March),
            (b"04".as_slice(), Month::April),
            (b"05".as_slice(), Month::May),
            (b"06".as_slice(), Month::June),
            (b"07".as_slice(), Month::July),
            (b"08".as_slice(), Month::August),
            (b"09".as_slice(), Month::September),
            (b"10".as_slice(), Month::October),
            (b"11".as_slice(), Month::November),
            (b"12".as_slice(), Month::December),
        ],
        true,
    )(input)
}

/// Parse a week number.
#[inline]
pub(crate) fn week(input: &[u8]) -> Option<ParsedItem<'_, NonZero<u8>>> {
    exactly_n_digits::<2, _>(input)
}

/// Parse a day of the month.
#[inline]
pub(crate) fn day(input: &[u8]) -> Option<ParsedItem<'_, NonZero<u8>>> {
    exactly_n_digits::<2, _>(input)
}

/// Parse a day of the week.
#[inline]
pub(crate) fn dayk(input: &[u8]) -> Option<ParsedItem<'_, Weekday>> {
    first_match(
        [
            (b"1".as_slice(), Weekday::Monday),
            (b"2".as_slice(), Weekday::Tuesday),
            (b"3".as_slice(), Weekday::Wednesday),
            (b"4".as_slice(), Weekday::Thursday),
            (b"5".as_slice(), Weekday::Friday),
            (b"6".as_slice(), Weekday::Saturday),
            (b"7".as_slice(), Weekday::Sunday),
        ],
        true,
    )(input)
}

/// Parse a day of the year.
#[inline]
pub(crate) fn dayo(input: &[u8]) -> Option<ParsedItem<'_, NonZero<u16>>> {
    exactly_n_digits::<3, _>(input)
}

/// Parse the hour.
#[inline]
pub(crate) fn hour(input: &[u8]) -> Option<ParsedItem<'_, u8>> {
    exactly_n_digits::<2, _>(input)
}

/// Parse the minute.
#[inline]
pub(crate) fn min(input: &[u8]) -> Option<ParsedItem<'_, u8>> {
    exactly_n_digits::<2, _>(input)
}

/// Parse a floating point number as its integer and optional fractional parts.
///
/// The number must have two digits before the decimal point. If a decimal point is present, at
/// least one digit must follow.
///
/// The return type is a tuple of the integer part and optional fraction part.
#[inline]
pub(crate) fn float(input: &[u8]) -> Option<ParsedItem<'_, (u8, Option<f64>)>> {
    // Two digits before the decimal.
    let ParsedItem(input, integer_part) = match input {
        [first_digit @ b'0'..=b'9', second_digit @ b'0'..=b'9', input @ ..] => {
            ParsedItem(input, (first_digit - b'0') * 10 + (second_digit - b'0'))
        }
        _ => return None,
    };

    if let Some(ParsedItem(input, ())) = decimal_sign(input) {
        // Mandatory post-decimal digit.
        let ParsedItem(mut input, mut fractional_part) =
            any_digit(input)?.map(|digit| ((digit - b'0') as f64) / 10.);

        let mut divisor = 10.;
        // Any number of subsequent digits.
        while let Some(ParsedItem(new_input, digit)) = any_digit(input) {
            input = new_input;
            divisor *= 10.;
            fractional_part += (digit - b'0') as f64 / divisor;
        }

        Some(ParsedItem(input, (integer_part, Some(fractional_part))))
    } else {
        Some(ParsedItem(input, (integer_part, None)))
    }
}

/// Parse a "decimal sign", which is either a comma or a period.
#[inline]
fn decimal_sign(input: &[u8]) -> Option<ParsedItem<'_, ()>> {
    ascii_char::<b'.'>(input).or_else(|| ascii_char::<b','>(input))
}
