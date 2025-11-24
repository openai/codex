//! Parsing implementations for all [`Component`](crate::format_description::Component)s.

use core::num::NonZero;

use num_conv::prelude::*;

use crate::convert::*;
use crate::format_description::modifier;
use crate::parsing::combinator::{
    any_digit, exactly_n_digits, exactly_n_digits_padded, first_match, n_to_m_digits,
    n_to_m_digits_padded, opt, sign,
};
use crate::parsing::ParsedItem;
use crate::{Month, Weekday};

/// Parse the "year" component of a `Date`.
pub(crate) fn parse_year(
    input: &[u8],
    modifiers: modifier::Year,
) -> Option<ParsedItem<'_, (i32, bool)>> {
    match modifiers.repr {
        modifier::YearRepr::Full => {
            let ParsedItem(input, sign) = opt(sign)(input);

            if let Some(sign) = sign {
                let ParsedItem(input, year) = if cfg!(feature = "large-dates")
                    && modifiers.range == modifier::YearRange::Extended
                {
                    n_to_m_digits_padded::<4, 6, u32>(modifiers.padding)(input)?
                } else {
                    exactly_n_digits_padded::<4, u32>(modifiers.padding)(input)?
                };

                Some(if sign == b'-' {
                    ParsedItem(input, (-year.cast_signed(), true))
                } else {
                    ParsedItem(input, (year.cast_signed(), false))
                })
            } else if modifiers.sign_is_mandatory {
                None
            } else {
                let ParsedItem(input, year) =
                    exactly_n_digits_padded::<4, u32>(modifiers.padding)(input)?;
                Some(ParsedItem(input, (year.cast_signed(), false)))
            }
        }
        modifier::YearRepr::Century => {
            let ParsedItem(input, sign) = opt(sign)(input);

            if let Some(sign) = sign {
                let ParsedItem(input, year) = if cfg!(feature = "large-dates")
                    && modifiers.range == modifier::YearRange::Extended
                {
                    n_to_m_digits_padded::<2, 4, u32>(modifiers.padding)(input)?
                } else {
                    exactly_n_digits_padded::<2, u32>(modifiers.padding)(input)?
                };

                Some(if sign == b'-' {
                    ParsedItem(input, (-year.cast_signed(), true))
                } else {
                    ParsedItem(input, (year.cast_signed(), false))
                })
            } else if modifiers.sign_is_mandatory {
                None
            } else {
                let ParsedItem(input, year) =
                    n_to_m_digits_padded::<1, 2, u32>(modifiers.padding)(input)?;
                Some(ParsedItem(input, (year.cast_signed(), false)))
            }
        }
        modifier::YearRepr::LastTwo => Some(
            exactly_n_digits_padded::<2, u32>(modifiers.padding)(input)?
                .map(|v| (v.cast_signed(), false)),
        ),
    }
}

/// Parse the "month" component of a `Date`.
pub(crate) fn parse_month(
    input: &[u8],
    modifiers: modifier::Month,
) -> Option<ParsedItem<'_, Month>> {
    use Month::*;
    let ParsedItem(remaining, value) = first_match(
        match modifiers.repr {
            modifier::MonthRepr::Numerical => {
                return exactly_n_digits_padded::<2, _>(modifiers.padding)(input)?
                    .flat_map(|n| Month::from_number(n).ok());
            }
            modifier::MonthRepr::Long => [
                (b"January".as_slice(), January),
                (b"February".as_slice(), February),
                (b"March".as_slice(), March),
                (b"April".as_slice(), April),
                (b"May".as_slice(), May),
                (b"June".as_slice(), June),
                (b"July".as_slice(), July),
                (b"August".as_slice(), August),
                (b"September".as_slice(), September),
                (b"October".as_slice(), October),
                (b"November".as_slice(), November),
                (b"December".as_slice(), December),
            ],
            modifier::MonthRepr::Short => [
                (b"Jan".as_slice(), January),
                (b"Feb".as_slice(), February),
                (b"Mar".as_slice(), March),
                (b"Apr".as_slice(), April),
                (b"May".as_slice(), May),
                (b"Jun".as_slice(), June),
                (b"Jul".as_slice(), July),
                (b"Aug".as_slice(), August),
                (b"Sep".as_slice(), September),
                (b"Oct".as_slice(), October),
                (b"Nov".as_slice(), November),
                (b"Dec".as_slice(), December),
            ],
        },
        modifiers.case_sensitive,
    )(input)?;
    Some(ParsedItem(remaining, value))
}

/// Parse the "week number" component of a `Date`.
pub(crate) fn parse_week_number(
    input: &[u8],
    modifiers: modifier::WeekNumber,
) -> Option<ParsedItem<'_, u8>> {
    exactly_n_digits_padded::<2, _>(modifiers.padding)(input)
}

/// Parse the "weekday" component of a `Date`.
pub(crate) fn parse_weekday(
    input: &[u8],
    modifiers: modifier::Weekday,
) -> Option<ParsedItem<'_, Weekday>> {
    first_match(
        match (modifiers.repr, modifiers.one_indexed) {
            (modifier::WeekdayRepr::Short, _) => [
                (b"Mon".as_slice(), Weekday::Monday),
                (b"Tue".as_slice(), Weekday::Tuesday),
                (b"Wed".as_slice(), Weekday::Wednesday),
                (b"Thu".as_slice(), Weekday::Thursday),
                (b"Fri".as_slice(), Weekday::Friday),
                (b"Sat".as_slice(), Weekday::Saturday),
                (b"Sun".as_slice(), Weekday::Sunday),
            ],
            (modifier::WeekdayRepr::Long, _) => [
                (b"Monday".as_slice(), Weekday::Monday),
                (b"Tuesday".as_slice(), Weekday::Tuesday),
                (b"Wednesday".as_slice(), Weekday::Wednesday),
                (b"Thursday".as_slice(), Weekday::Thursday),
                (b"Friday".as_slice(), Weekday::Friday),
                (b"Saturday".as_slice(), Weekday::Saturday),
                (b"Sunday".as_slice(), Weekday::Sunday),
            ],
            (modifier::WeekdayRepr::Sunday, false) => [
                (b"1".as_slice(), Weekday::Monday),
                (b"2".as_slice(), Weekday::Tuesday),
                (b"3".as_slice(), Weekday::Wednesday),
                (b"4".as_slice(), Weekday::Thursday),
                (b"5".as_slice(), Weekday::Friday),
                (b"6".as_slice(), Weekday::Saturday),
                (b"0".as_slice(), Weekday::Sunday),
            ],
            (modifier::WeekdayRepr::Sunday, true) => [
                (b"2".as_slice(), Weekday::Monday),
                (b"3".as_slice(), Weekday::Tuesday),
                (b"4".as_slice(), Weekday::Wednesday),
                (b"5".as_slice(), Weekday::Thursday),
                (b"6".as_slice(), Weekday::Friday),
                (b"7".as_slice(), Weekday::Saturday),
                (b"1".as_slice(), Weekday::Sunday),
            ],
            (modifier::WeekdayRepr::Monday, false) => [
                (b"0".as_slice(), Weekday::Monday),
                (b"1".as_slice(), Weekday::Tuesday),
                (b"2".as_slice(), Weekday::Wednesday),
                (b"3".as_slice(), Weekday::Thursday),
                (b"4".as_slice(), Weekday::Friday),
                (b"5".as_slice(), Weekday::Saturday),
                (b"6".as_slice(), Weekday::Sunday),
            ],
            (modifier::WeekdayRepr::Monday, true) => [
                (b"1".as_slice(), Weekday::Monday),
                (b"2".as_slice(), Weekday::Tuesday),
                (b"3".as_slice(), Weekday::Wednesday),
                (b"4".as_slice(), Weekday::Thursday),
                (b"5".as_slice(), Weekday::Friday),
                (b"6".as_slice(), Weekday::Saturday),
                (b"7".as_slice(), Weekday::Sunday),
            ],
        },
        modifiers.case_sensitive,
    )(input)
}

/// Parse the "ordinal" component of a `Date`.
#[inline]
pub(crate) fn parse_ordinal(
    input: &[u8],
    modifiers: modifier::Ordinal,
) -> Option<ParsedItem<'_, NonZero<u16>>> {
    exactly_n_digits_padded::<3, _>(modifiers.padding)(input)
}

/// Parse the "day" component of a `Date`.
#[inline]
pub(crate) fn parse_day(
    input: &[u8],
    modifiers: modifier::Day,
) -> Option<ParsedItem<'_, NonZero<u8>>> {
    exactly_n_digits_padded::<2, _>(modifiers.padding)(input)
}

/// Indicate whether the hour is "am" or "pm".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Period {
    #[allow(clippy::missing_docs_in_private_items)]
    Am,
    #[allow(clippy::missing_docs_in_private_items)]
    Pm,
}

/// Parse the "hour" component of a `Time`.
#[inline]
pub(crate) fn parse_hour(input: &[u8], modifiers: modifier::Hour) -> Option<ParsedItem<'_, u8>> {
    exactly_n_digits_padded::<2, _>(modifiers.padding)(input)
}

/// Parse the "minute" component of a `Time`.
#[inline]
pub(crate) fn parse_minute(
    input: &[u8],
    modifiers: modifier::Minute,
) -> Option<ParsedItem<'_, u8>> {
    exactly_n_digits_padded::<2, _>(modifiers.padding)(input)
}

/// Parse the "second" component of a `Time`.
#[inline]
pub(crate) fn parse_second(
    input: &[u8],
    modifiers: modifier::Second,
) -> Option<ParsedItem<'_, u8>> {
    exactly_n_digits_padded::<2, _>(modifiers.padding)(input)
}

/// Parse the "period" component of a `Time`. Required if the hour is on a 12-hour clock.
#[inline]
pub(crate) fn parse_period(
    input: &[u8],
    modifiers: modifier::Period,
) -> Option<ParsedItem<'_, Period>> {
    first_match(
        if modifiers.is_uppercase {
            [
                (b"AM".as_slice(), Period::Am),
                (b"PM".as_slice(), Period::Pm),
            ]
        } else {
            [
                (b"am".as_slice(), Period::Am),
                (b"pm".as_slice(), Period::Pm),
            ]
        },
        modifiers.case_sensitive,
    )(input)
}

/// Parse the "subsecond" component of a `Time`.
pub(crate) fn parse_subsecond(
    input: &[u8],
    modifiers: modifier::Subsecond,
) -> Option<ParsedItem<'_, u32>> {
    use modifier::SubsecondDigits::*;
    Some(match modifiers.digits {
        One => exactly_n_digits::<1, u32>(input)?.map(|v| v * 100_000_000),
        Two => exactly_n_digits::<2, u32>(input)?.map(|v| v * 10_000_000),
        Three => exactly_n_digits::<3, u32>(input)?.map(|v| v * 1_000_000),
        Four => exactly_n_digits::<4, u32>(input)?.map(|v| v * 100_000),
        Five => exactly_n_digits::<5, u32>(input)?.map(|v| v * 10_000),
        Six => exactly_n_digits::<6, u32>(input)?.map(|v| v * 1_000),
        Seven => exactly_n_digits::<7, u32>(input)?.map(|v| v * 100),
        Eight => exactly_n_digits::<8, u32>(input)?.map(|v| v * 10),
        Nine => exactly_n_digits::<9, _>(input)?,
        OneOrMore => {
            let ParsedItem(mut input, mut value) =
                any_digit(input)?.map(|v| (v - b'0').extend::<u32>() * 100_000_000);

            let mut multiplier = 10_000_000;
            while let Some(ParsedItem(new_input, digit)) = any_digit(input) {
                value += (digit - b'0').extend::<u32>() * multiplier;
                input = new_input;
                multiplier /= 10;
            }

            ParsedItem(input, value)
        }
    })
}

/// Parse the "hour" component of a `UtcOffset`.
///
/// Returns the value and whether the value is negative. This is used for when "-0" is parsed.
pub(crate) fn parse_offset_hour(
    input: &[u8],
    modifiers: modifier::OffsetHour,
) -> Option<ParsedItem<'_, (i8, bool)>> {
    let ParsedItem(input, sign) = opt(sign)(input);
    let ParsedItem(input, hour) = exactly_n_digits_padded::<2, u8>(modifiers.padding)(input)?;
    match sign {
        Some(b'-') => Some(ParsedItem(input, (-hour.cast_signed(), true))),
        None if modifiers.sign_is_mandatory => None,
        _ => Some(ParsedItem(input, (hour.cast_signed(), false))),
    }
}

/// Parse the "minute" component of a `UtcOffset`.
#[inline]
pub(crate) fn parse_offset_minute(
    input: &[u8],
    modifiers: modifier::OffsetMinute,
) -> Option<ParsedItem<'_, i8>> {
    Some(
        exactly_n_digits_padded::<2, u8>(modifiers.padding)(input)?
            .map(|offset_minute| offset_minute.cast_signed()),
    )
}

/// Parse the "second" component of a `UtcOffset`.
#[inline]
pub(crate) fn parse_offset_second(
    input: &[u8],
    modifiers: modifier::OffsetSecond,
) -> Option<ParsedItem<'_, i8>> {
    Some(
        exactly_n_digits_padded::<2, u8>(modifiers.padding)(input)?
            .map(|offset_second| offset_second.cast_signed()),
    )
}

/// Ignore the given number of bytes.
#[inline]
pub(crate) fn parse_ignore(
    input: &[u8],
    modifiers: modifier::Ignore,
) -> Option<ParsedItem<'_, ()>> {
    let modifier::Ignore { count } = modifiers;
    let input = input.get((count.get().extend())..)?;
    Some(ParsedItem(input, ()))
}

/// Parse the Unix timestamp component.
pub(crate) fn parse_unix_timestamp(
    input: &[u8],
    modifiers: modifier::UnixTimestamp,
) -> Option<ParsedItem<'_, i128>> {
    let ParsedItem(input, sign) = opt(sign)(input);
    let ParsedItem(input, nano_timestamp) = match modifiers.precision {
        modifier::UnixTimestampPrecision::Second => {
            n_to_m_digits::<1, 14, u128>(input)?.map(|val| val * Nanosecond::per_t::<u128>(Second))
        }
        modifier::UnixTimestampPrecision::Millisecond => n_to_m_digits::<1, 17, u128>(input)?
            .map(|val| val * Nanosecond::per_t::<u128>(Millisecond)),
        modifier::UnixTimestampPrecision::Microsecond => n_to_m_digits::<1, 20, u128>(input)?
            .map(|val| val * Nanosecond::per_t::<u128>(Microsecond)),
        modifier::UnixTimestampPrecision::Nanosecond => n_to_m_digits::<1, 23, _>(input)?,
    };

    match sign {
        Some(b'-') => Some(ParsedItem(input, -nano_timestamp.cast_signed())),
        None if modifiers.sign_is_mandatory => None,
        _ => Some(ParsedItem(input, nano_timestamp.cast_signed())),
    }
}

/// Parse the `end` component, which represents the end of input. If any input is remaining, `None`
/// is returned.
#[inline]
pub(crate) const fn parse_end(input: &[u8], end: modifier::End) -> Option<ParsedItem<'_, ()>> {
    let modifier::End {} = end;

    if input.is_empty() {
        Some(ParsedItem(input, ()))
    } else {
        None
    }
}
