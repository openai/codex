//! A trait that can be used to parse an item from an input.

use core::ops::Deref;

use num_conv::prelude::*;

use crate::error::TryFromParsed;
use crate::format_description::well_known::iso8601::EncodedConfig;
use crate::format_description::well_known::{Iso8601, Rfc2822, Rfc3339};
use crate::format_description::BorrowedFormatItem;
#[cfg(feature = "alloc")]
use crate::format_description::OwnedFormatItem;
use crate::internal_macros::bug;
use crate::parsing::{Parsed, ParsedItem};
use crate::{error, Date, Month, OffsetDateTime, Time, UtcOffset, Weekday};

/// A type that can be parsed.
#[cfg_attr(docsrs, doc(notable_trait))]
#[doc(alias = "Parseable")]
pub trait Parsable: sealed::Sealed {}
impl Parsable for BorrowedFormatItem<'_> {}
impl Parsable for [BorrowedFormatItem<'_>] {}
#[cfg(feature = "alloc")]
impl Parsable for OwnedFormatItem {}
#[cfg(feature = "alloc")]
impl Parsable for [OwnedFormatItem] {}
impl Parsable for Rfc2822 {}
impl Parsable for Rfc3339 {}
impl<const CONFIG: EncodedConfig> Parsable for Iso8601<CONFIG> {}
impl<T: Deref> Parsable for T where T::Target: Parsable {}

/// Seal the trait to prevent downstream users from implementing it, while still allowing it to
/// exist in generic bounds.
mod sealed {
    use super::*;
    use crate::{PrimitiveDateTime, UtcDateTime};

    /// Parse the item using a format description and an input.
    pub trait Sealed {
        /// Parse the item into the provided [`Parsed`] struct.
        ///
        /// This method can be used to parse a single component without parsing the full value.
        fn parse_into<'a>(
            &self,
            input: &'a [u8],
            parsed: &mut Parsed,
        ) -> Result<&'a [u8], error::Parse>;

        /// Parse the item into a new [`Parsed`] struct.
        ///
        /// This method can only be used to parse a complete value of a type. If any characters
        /// remain after parsing, an error will be returned.
        #[inline]
        fn parse(&self, input: &[u8]) -> Result<Parsed, error::Parse> {
            let mut parsed = Parsed::new();
            if self.parse_into(input, &mut parsed)?.is_empty() {
                Ok(parsed)
            } else {
                Err(error::Parse::ParseFromDescription(
                    error::ParseFromDescription::UnexpectedTrailingCharacters,
                ))
            }
        }

        /// Parse a [`Date`] from the format description.
        #[inline]
        fn parse_date(&self, input: &[u8]) -> Result<Date, error::Parse> {
            Ok(self.parse(input)?.try_into()?)
        }

        /// Parse a [`Time`] from the format description.
        #[inline]
        fn parse_time(&self, input: &[u8]) -> Result<Time, error::Parse> {
            Ok(self.parse(input)?.try_into()?)
        }

        /// Parse a [`UtcOffset`] from the format description.
        #[inline]
        fn parse_offset(&self, input: &[u8]) -> Result<UtcOffset, error::Parse> {
            Ok(self.parse(input)?.try_into()?)
        }

        /// Parse a [`PrimitiveDateTime`] from the format description.
        #[inline]
        fn parse_primitive_date_time(
            &self,
            input: &[u8],
        ) -> Result<PrimitiveDateTime, error::Parse> {
            Ok(self.parse(input)?.try_into()?)
        }

        /// Parse a [`UtcDateTime`] from the format description.
        #[inline]
        fn parse_utc_date_time(&self, input: &[u8]) -> Result<UtcDateTime, error::Parse> {
            Ok(self.parse(input)?.try_into()?)
        }

        /// Parse a [`OffsetDateTime`] from the format description.
        #[inline]
        fn parse_offset_date_time(&self, input: &[u8]) -> Result<OffsetDateTime, error::Parse> {
            Ok(self.parse(input)?.try_into()?)
        }
    }
}

impl sealed::Sealed for BorrowedFormatItem<'_> {
    #[inline]
    fn parse_into<'a>(
        &self,
        input: &'a [u8],
        parsed: &mut Parsed,
    ) -> Result<&'a [u8], error::Parse> {
        Ok(parsed.parse_item(input, self)?)
    }
}

impl sealed::Sealed for [BorrowedFormatItem<'_>] {
    #[inline]
    fn parse_into<'a>(
        &self,
        input: &'a [u8],
        parsed: &mut Parsed,
    ) -> Result<&'a [u8], error::Parse> {
        Ok(parsed.parse_items(input, self)?)
    }
}

#[cfg(feature = "alloc")]
impl sealed::Sealed for OwnedFormatItem {
    #[inline]
    fn parse_into<'a>(
        &self,
        input: &'a [u8],
        parsed: &mut Parsed,
    ) -> Result<&'a [u8], error::Parse> {
        Ok(parsed.parse_item(input, self)?)
    }
}

#[cfg(feature = "alloc")]
impl sealed::Sealed for [OwnedFormatItem] {
    #[inline]
    fn parse_into<'a>(
        &self,
        input: &'a [u8],
        parsed: &mut Parsed,
    ) -> Result<&'a [u8], error::Parse> {
        Ok(parsed.parse_items(input, self)?)
    }
}

impl<T> sealed::Sealed for T
where
    T: Deref<Target: sealed::Sealed>,
{
    #[inline]
    fn parse_into<'a>(
        &self,
        input: &'a [u8],
        parsed: &mut Parsed,
    ) -> Result<&'a [u8], error::Parse> {
        self.deref().parse_into(input, parsed)
    }
}

impl sealed::Sealed for Rfc2822 {
    fn parse_into<'a>(
        &self,
        input: &'a [u8],
        parsed: &mut Parsed,
    ) -> Result<&'a [u8], error::Parse> {
        use crate::error::ParseFromDescription::{InvalidComponent, InvalidLiteral};
        use crate::parsing::combinator::rfc::rfc2822::{cfws, fws};
        use crate::parsing::combinator::{
            ascii_char, exactly_n_digits, first_match, n_to_m_digits, opt, sign,
        };

        let colon = ascii_char::<b':'>;
        let comma = ascii_char::<b','>;

        let input = opt(cfws)(input).into_inner();
        let weekday = first_match(
            [
                (b"Mon".as_slice(), Weekday::Monday),
                (b"Tue".as_slice(), Weekday::Tuesday),
                (b"Wed".as_slice(), Weekday::Wednesday),
                (b"Thu".as_slice(), Weekday::Thursday),
                (b"Fri".as_slice(), Weekday::Friday),
                (b"Sat".as_slice(), Weekday::Saturday),
                (b"Sun".as_slice(), Weekday::Sunday),
            ],
            false,
        )(input);
        let input = if let Some(item) = weekday {
            let input = item
                .consume_value(|value| parsed.set_weekday(value))
                .ok_or(InvalidComponent("weekday"))?;
            let input = comma(input).ok_or(InvalidLiteral)?.into_inner();
            opt(cfws)(input).into_inner()
        } else {
            input
        };
        let input = n_to_m_digits::<1, 2, _>(input)
            .and_then(|item| item.consume_value(|value| parsed.set_day(value)))
            .ok_or(InvalidComponent("day"))?;
        let input = cfws(input).ok_or(InvalidLiteral)?.into_inner();
        let input = first_match(
            [
                (b"Jan".as_slice(), Month::January),
                (b"Feb".as_slice(), Month::February),
                (b"Mar".as_slice(), Month::March),
                (b"Apr".as_slice(), Month::April),
                (b"May".as_slice(), Month::May),
                (b"Jun".as_slice(), Month::June),
                (b"Jul".as_slice(), Month::July),
                (b"Aug".as_slice(), Month::August),
                (b"Sep".as_slice(), Month::September),
                (b"Oct".as_slice(), Month::October),
                (b"Nov".as_slice(), Month::November),
                (b"Dec".as_slice(), Month::December),
            ],
            false,
        )(input)
        .and_then(|item| item.consume_value(|value| parsed.set_month(value)))
        .ok_or(InvalidComponent("month"))?;
        let input = cfws(input).ok_or(InvalidLiteral)?.into_inner();
        let input = match exactly_n_digits::<4, u32>(input) {
            Some(item) => {
                let input = item
                    .flat_map(|year| if year >= 1900 { Some(year) } else { None })
                    .and_then(|item| {
                        item.consume_value(|value| parsed.set_year(value.cast_signed()))
                    })
                    .ok_or(InvalidComponent("year"))?;
                fws(input).ok_or(InvalidLiteral)?.into_inner()
            }
            None => {
                let input = exactly_n_digits::<2, u32>(input)
                    .and_then(|item| {
                        item.map(|year| if year < 50 { year + 2000 } else { year + 1900 })
                            .map(|year| year.cast_signed())
                            .consume_value(|value| parsed.set_year(value))
                    })
                    .ok_or(InvalidComponent("year"))?;
                cfws(input).ok_or(InvalidLiteral)?.into_inner()
            }
        };

        let input = exactly_n_digits::<2, _>(input)
            .and_then(|item| item.consume_value(|value| parsed.set_hour_24(value)))
            .ok_or(InvalidComponent("hour"))?;
        let input = opt(cfws)(input).into_inner();
        let input = colon(input).ok_or(InvalidLiteral)?.into_inner();
        let input = opt(cfws)(input).into_inner();
        let input = exactly_n_digits::<2, _>(input)
            .and_then(|item| item.consume_value(|value| parsed.set_minute(value)))
            .ok_or(InvalidComponent("minute"))?;

        let input = if let Some(input) = colon(opt(cfws)(input).into_inner()) {
            let input = input.into_inner(); // discard the colon
            let input = opt(cfws)(input).into_inner();
            let input = exactly_n_digits::<2, _>(input)
                .and_then(|item| item.consume_value(|value| parsed.set_second(value)))
                .ok_or(InvalidComponent("second"))?;
            cfws(input).ok_or(InvalidLiteral)?.into_inner()
        } else {
            cfws(input).ok_or(InvalidLiteral)?.into_inner()
        };

        // The RFC explicitly allows leap seconds.
        parsed.leap_second_allowed = true;

        #[expect(
            clippy::unnecessary_lazy_evaluations,
            reason = "rust-lang/rust-clippy#8522"
        )]
        let zone_literal = first_match(
            [
                (b"UT".as_slice(), 0),
                (b"GMT".as_slice(), 0),
                (b"EST".as_slice(), -5),
                (b"EDT".as_slice(), -4),
                (b"CST".as_slice(), -6),
                (b"CDT".as_slice(), -5),
                (b"MST".as_slice(), -7),
                (b"MDT".as_slice(), -6),
                (b"PST".as_slice(), -8),
                (b"PDT".as_slice(), -7),
            ],
            false,
        )(input)
        .or_else(|| match input {
            [b'a'..=b'i' | b'k'..=b'z' | b'A'..=b'I' | b'K'..=b'Z', rest @ ..] => {
                Some(ParsedItem(rest, 0))
            }
            _ => None,
        });
        if let Some(zone_literal) = zone_literal {
            let input = zone_literal
                .consume_value(|value| parsed.set_offset_hour(value))
                .ok_or(InvalidComponent("offset hour"))?;
            parsed
                .set_offset_minute_signed(0)
                .ok_or(InvalidComponent("offset minute"))?;
            parsed
                .set_offset_second_signed(0)
                .ok_or(InvalidComponent("offset second"))?;
            return Ok(input);
        }

        let ParsedItem(input, offset_sign) = sign(input).ok_or(InvalidComponent("offset hour"))?;
        let input = exactly_n_digits::<2, u8>(input)
            .and_then(|item| {
                item.map(|offset_hour| {
                    if offset_sign == b'-' {
                        -offset_hour.cast_signed()
                    } else {
                        offset_hour.cast_signed()
                    }
                })
                .consume_value(|value| parsed.set_offset_hour(value))
            })
            .ok_or(InvalidComponent("offset hour"))?;
        let input = exactly_n_digits::<2, u8>(input)
            .and_then(|item| {
                item.consume_value(|value| parsed.set_offset_minute_signed(value.cast_signed()))
            })
            .ok_or(InvalidComponent("offset minute"))?;

        let input = opt(cfws)(input).into_inner();

        Ok(input)
    }

    fn parse_offset_date_time(&self, input: &[u8]) -> Result<OffsetDateTime, error::Parse> {
        use crate::error::ParseFromDescription::{InvalidComponent, InvalidLiteral};
        use crate::parsing::combinator::rfc::rfc2822::{cfws, fws};
        use crate::parsing::combinator::{
            ascii_char, exactly_n_digits, first_match, n_to_m_digits, opt, sign,
        };

        let colon = ascii_char::<b':'>;
        let comma = ascii_char::<b','>;

        let input = opt(cfws)(input).into_inner();
        // This parses the weekday, but we don't actually use the value anywhere. Because of this,
        // just return `()` to avoid unnecessary generated code.
        let weekday = first_match(
            [
                (b"Mon".as_slice(), ()),
                (b"Tue".as_slice(), ()),
                (b"Wed".as_slice(), ()),
                (b"Thu".as_slice(), ()),
                (b"Fri".as_slice(), ()),
                (b"Sat".as_slice(), ()),
                (b"Sun".as_slice(), ()),
            ],
            false,
        )(input);
        let input = if let Some(item) = weekday {
            let input = item.into_inner();
            let input = comma(input).ok_or(InvalidLiteral)?.into_inner();
            opt(cfws)(input).into_inner()
        } else {
            input
        };
        let ParsedItem(input, day) =
            n_to_m_digits::<1, 2, _>(input).ok_or(InvalidComponent("day"))?;
        let input = cfws(input).ok_or(InvalidLiteral)?.into_inner();
        let ParsedItem(input, month) = first_match(
            [
                (b"Jan".as_slice(), Month::January),
                (b"Feb".as_slice(), Month::February),
                (b"Mar".as_slice(), Month::March),
                (b"Apr".as_slice(), Month::April),
                (b"May".as_slice(), Month::May),
                (b"Jun".as_slice(), Month::June),
                (b"Jul".as_slice(), Month::July),
                (b"Aug".as_slice(), Month::August),
                (b"Sep".as_slice(), Month::September),
                (b"Oct".as_slice(), Month::October),
                (b"Nov".as_slice(), Month::November),
                (b"Dec".as_slice(), Month::December),
            ],
            false,
        )(input)
        .ok_or(InvalidComponent("month"))?;
        let input = cfws(input).ok_or(InvalidLiteral)?.into_inner();
        let (input, year) = match exactly_n_digits::<4, u32>(input) {
            Some(item) => {
                let ParsedItem(input, year) = item
                    .flat_map(|year| if year >= 1900 { Some(year) } else { None })
                    .ok_or(InvalidComponent("year"))?;
                let input = fws(input).ok_or(InvalidLiteral)?.into_inner();
                (input, year)
            }
            None => {
                let ParsedItem(input, year) = exactly_n_digits::<2, u32>(input)
                    .map(|item| item.map(|year| if year < 50 { year + 2000 } else { year + 1900 }))
                    .ok_or(InvalidComponent("year"))?;
                let input = cfws(input).ok_or(InvalidLiteral)?.into_inner();
                (input, year)
            }
        };

        let ParsedItem(input, hour) =
            exactly_n_digits::<2, _>(input).ok_or(InvalidComponent("hour"))?;
        let input = opt(cfws)(input).into_inner();
        let input = colon(input).ok_or(InvalidLiteral)?.into_inner();
        let input = opt(cfws)(input).into_inner();
        let ParsedItem(input, minute) =
            exactly_n_digits::<2, _>(input).ok_or(InvalidComponent("minute"))?;

        let (input, mut second) = if let Some(input) = colon(opt(cfws)(input).into_inner()) {
            let input = input.into_inner(); // discard the colon
            let input = opt(cfws)(input).into_inner();
            let ParsedItem(input, second) =
                exactly_n_digits::<2, _>(input).ok_or(InvalidComponent("second"))?;
            let input = cfws(input).ok_or(InvalidLiteral)?.into_inner();
            (input, second)
        } else {
            (cfws(input).ok_or(InvalidLiteral)?.into_inner(), 0)
        };

        #[expect(
            clippy::unnecessary_lazy_evaluations,
            reason = "rust-lang/rust-clippy#8522"
        )]
        let zone_literal = first_match(
            [
                (b"UT".as_slice(), 0),
                (b"GMT".as_slice(), 0),
                (b"EST".as_slice(), -5),
                (b"EDT".as_slice(), -4),
                (b"CST".as_slice(), -6),
                (b"CDT".as_slice(), -5),
                (b"MST".as_slice(), -7),
                (b"MDT".as_slice(), -6),
                (b"PST".as_slice(), -8),
                (b"PDT".as_slice(), -7),
            ],
            false,
        )(input)
        .or_else(|| match input {
            [b'a'..=b'i' | b'k'..=b'z' | b'A'..=b'I' | b'K'..=b'Z', rest @ ..] => {
                Some(ParsedItem(rest, 0))
            }
            _ => None,
        });

        let (input, offset_hour, offset_minute) = if let Some(zone_literal) = zone_literal {
            let ParsedItem(input, offset_hour) = zone_literal;
            (input, offset_hour, 0)
        } else {
            let ParsedItem(input, offset_sign) =
                sign(input).ok_or(InvalidComponent("offset hour"))?;
            let ParsedItem(input, offset_hour) = exactly_n_digits::<2, u8>(input)
                .map(|item| {
                    item.map(|offset_hour| {
                        if offset_sign == b'-' {
                            -offset_hour.cast_signed()
                        } else {
                            offset_hour.cast_signed()
                        }
                    })
                })
                .ok_or(InvalidComponent("offset hour"))?;
            let ParsedItem(input, offset_minute) =
                exactly_n_digits::<2, u8>(input).ok_or(InvalidComponent("offset minute"))?;
            (input, offset_hour, offset_minute.cast_signed())
        };

        let input = opt(cfws)(input).into_inner();

        if !input.is_empty() {
            return Err(error::Parse::ParseFromDescription(
                error::ParseFromDescription::UnexpectedTrailingCharacters,
            ));
        }

        let mut nanosecond = 0;
        let leap_second_input = if second == 60 {
            second = 59;
            nanosecond = 999_999_999;
            true
        } else {
            false
        };

        let dt = (|| {
            let date = Date::from_calendar_date(year.cast_signed(), month, day)?;
            let time = Time::from_hms_nano(hour, minute, second, nanosecond)?;
            let offset = UtcOffset::from_hms(offset_hour, offset_minute, 0)?;
            Ok(OffsetDateTime::new_in_offset(date, time, offset))
        })()
        .map_err(TryFromParsed::ComponentRange)?;

        if leap_second_input && !dt.is_valid_leap_second_stand_in() {
            return Err(error::Parse::TryFromParsed(TryFromParsed::ComponentRange(
                error::ComponentRange {
                    name: "second",
                    minimum: 0,
                    maximum: 59,
                    value: 60,
                    conditional_message: Some("because leap seconds are not supported"),
                },
            )));
        }

        Ok(dt)
    }
}

impl sealed::Sealed for Rfc3339 {
    fn parse_into<'a>(
        &self,
        input: &'a [u8],
        parsed: &mut Parsed,
    ) -> Result<&'a [u8], error::Parse> {
        use crate::error::ParseFromDescription::{InvalidComponent, InvalidLiteral};
        use crate::parsing::combinator::{
            any_digit, ascii_char, ascii_char_ignore_case, exactly_n_digits, sign,
        };

        let dash = ascii_char::<b'-'>;
        let colon = ascii_char::<b':'>;

        let input = exactly_n_digits::<4, u32>(input)
            .and_then(|item| item.consume_value(|value| parsed.set_year(value.cast_signed())))
            .ok_or(InvalidComponent("year"))?;
        let input = dash(input).ok_or(InvalidLiteral)?.into_inner();
        let input = exactly_n_digits::<2, _>(input)
            .and_then(|item| item.flat_map(|value| Month::from_number(value).ok()))
            .and_then(|item| item.consume_value(|value| parsed.set_month(value)))
            .ok_or(InvalidComponent("month"))?;
        let input = dash(input).ok_or(InvalidLiteral)?.into_inner();
        let input = exactly_n_digits::<2, _>(input)
            .and_then(|item| item.consume_value(|value| parsed.set_day(value)))
            .ok_or(InvalidComponent("day"))?;

        // RFC3339 allows any separator, not just `T`, not just `space`.
        // cf. Section 5.6: Internet Date/Time Format:
        //   NOTE: ISO 8601 defines date and time separated by "T".
        //   Applications using this syntax may choose, for the sake of
        //   readability, to specify a full-date and full-time separated by
        //   (say) a space character.
        // Specifically, rusqlite uses space separators.
        let input = input.get(1..).ok_or(InvalidComponent("separator"))?;

        let input = exactly_n_digits::<2, _>(input)
            .and_then(|item| item.consume_value(|value| parsed.set_hour_24(value)))
            .ok_or(InvalidComponent("hour"))?;
        let input = colon(input).ok_or(InvalidLiteral)?.into_inner();
        let input = exactly_n_digits::<2, _>(input)
            .and_then(|item| item.consume_value(|value| parsed.set_minute(value)))
            .ok_or(InvalidComponent("minute"))?;
        let input = colon(input).ok_or(InvalidLiteral)?.into_inner();
        let input = exactly_n_digits::<2, _>(input)
            .and_then(|item| item.consume_value(|value| parsed.set_second(value)))
            .ok_or(InvalidComponent("second"))?;
        let input = if let Some(ParsedItem(input, ())) = ascii_char::<b'.'>(input) {
            let ParsedItem(mut input, mut value) = any_digit(input)
                .ok_or(InvalidComponent("subsecond"))?
                .map(|v| (v - b'0').extend::<u32>() * 100_000_000);

            let mut multiplier = 10_000_000;
            while let Some(ParsedItem(new_input, digit)) = any_digit(input) {
                value += (digit - b'0').extend::<u32>() * multiplier;
                input = new_input;
                multiplier /= 10;
            }

            parsed
                .set_subsecond(value)
                .ok_or(InvalidComponent("subsecond"))?;
            input
        } else {
            input
        };

        // The RFC explicitly allows leap seconds.
        parsed.leap_second_allowed = true;

        if let Some(ParsedItem(input, ())) = ascii_char_ignore_case::<b'Z'>(input) {
            parsed
                .set_offset_hour(0)
                .ok_or(InvalidComponent("offset hour"))?;
            parsed
                .set_offset_minute_signed(0)
                .ok_or(InvalidComponent("offset minute"))?;
            parsed
                .set_offset_second_signed(0)
                .ok_or(InvalidComponent("offset second"))?;
            return Ok(input);
        }

        let ParsedItem(input, offset_sign) = sign(input).ok_or(InvalidComponent("offset hour"))?;
        let input = exactly_n_digits::<2, u8>(input)
            .and_then(|item| {
                item.filter(|&offset_hour| offset_hour <= 23)?
                    .map(|offset_hour| {
                        if offset_sign == b'-' {
                            -offset_hour.cast_signed()
                        } else {
                            offset_hour.cast_signed()
                        }
                    })
                    .consume_value(|value| parsed.set_offset_hour(value))
            })
            .ok_or(InvalidComponent("offset hour"))?;
        let input = colon(input).ok_or(InvalidLiteral)?.into_inner();
        let input = exactly_n_digits::<2, u8>(input)
            .and_then(|item| {
                item.map(|offset_minute| {
                    if offset_sign == b'-' {
                        -offset_minute.cast_signed()
                    } else {
                        offset_minute.cast_signed()
                    }
                })
                .consume_value(|value| parsed.set_offset_minute_signed(value))
            })
            .ok_or(InvalidComponent("offset minute"))?;

        Ok(input)
    }

    fn parse_offset_date_time(&self, input: &[u8]) -> Result<OffsetDateTime, error::Parse> {
        use crate::error::ParseFromDescription::{InvalidComponent, InvalidLiteral};
        use crate::parsing::combinator::{
            any_digit, ascii_char, ascii_char_ignore_case, exactly_n_digits, sign,
        };

        let dash = ascii_char::<b'-'>;
        let colon = ascii_char::<b':'>;

        let ParsedItem(input, year) =
            exactly_n_digits::<4, u32>(input).ok_or(InvalidComponent("year"))?;
        let input = dash(input).ok_or(InvalidLiteral)?.into_inner();
        let ParsedItem(input, month) =
            exactly_n_digits::<2, _>(input).ok_or(InvalidComponent("month"))?;
        let input = dash(input).ok_or(InvalidLiteral)?.into_inner();
        let ParsedItem(input, day) =
            exactly_n_digits::<2, _>(input).ok_or(InvalidComponent("day"))?;

        // RFC3339 allows any separator, not just `T`, not just `space`.
        // cf. Section 5.6: Internet Date/Time Format:
        //   NOTE: ISO 8601 defines date and time separated by "T".
        //   Applications using this syntax may choose, for the sake of
        //   readability, to specify a full-date and full-time separated by
        //   (say) a space character.
        // Specifically, rusqlite uses space separators.
        let input = input.get(1..).ok_or(InvalidComponent("separator"))?;

        let ParsedItem(input, hour) =
            exactly_n_digits::<2, _>(input).ok_or(InvalidComponent("hour"))?;
        let input = colon(input).ok_or(InvalidLiteral)?.into_inner();
        let ParsedItem(input, minute) =
            exactly_n_digits::<2, _>(input).ok_or(InvalidComponent("minute"))?;
        let input = colon(input).ok_or(InvalidLiteral)?.into_inner();
        let ParsedItem(input, mut second) =
            exactly_n_digits::<2, _>(input).ok_or(InvalidComponent("second"))?;
        let ParsedItem(input, mut nanosecond) =
            if let Some(ParsedItem(input, ())) = ascii_char::<b'.'>(input) {
                let ParsedItem(mut input, mut value) = any_digit(input)
                    .ok_or(InvalidComponent("subsecond"))?
                    .map(|v| (v - b'0').extend::<u32>() * 100_000_000);

                let mut multiplier = 10_000_000;
                while let Some(ParsedItem(new_input, digit)) = any_digit(input) {
                    value += (digit - b'0').extend::<u32>() * multiplier;
                    input = new_input;
                    multiplier /= 10;
                }

                ParsedItem(input, value)
            } else {
                ParsedItem(input, 0)
            };
        let ParsedItem(input, offset) = {
            if let Some(ParsedItem(input, ())) = ascii_char_ignore_case::<b'Z'>(input) {
                ParsedItem(input, UtcOffset::UTC)
            } else {
                let ParsedItem(input, offset_sign) =
                    sign(input).ok_or(InvalidComponent("offset hour"))?;
                let ParsedItem(input, offset_hour) = exactly_n_digits::<2, u8>(input)
                    .and_then(|parsed| parsed.filter(|&offset_hour| offset_hour <= 23))
                    .ok_or(InvalidComponent("offset hour"))?;
                let input = colon(input).ok_or(InvalidLiteral)?.into_inner();
                let ParsedItem(input, offset_minute) =
                    exactly_n_digits::<2, u8>(input).ok_or(InvalidComponent("offset minute"))?;
                UtcOffset::from_hms(
                    if offset_sign == b'-' {
                        -offset_hour.cast_signed()
                    } else {
                        offset_hour.cast_signed()
                    },
                    if offset_sign == b'-' {
                        -offset_minute.cast_signed()
                    } else {
                        offset_minute.cast_signed()
                    },
                    0,
                )
                .map(|offset| ParsedItem(input, offset))
                .map_err(|mut err| {
                    // Provide the user a more accurate error.
                    if err.name == "hours" {
                        err.name = "offset hour";
                    } else if err.name == "minutes" {
                        err.name = "offset minute";
                    }
                    err
                })
                .map_err(TryFromParsed::ComponentRange)?
            }
        };

        if !input.is_empty() {
            return Err(error::Parse::ParseFromDescription(
                error::ParseFromDescription::UnexpectedTrailingCharacters,
            ));
        }

        // The RFC explicitly permits leap seconds. We don't currently support them, so treat it as
        // the preceding nanosecond. However, leap seconds can only occur as the last second of the
        // month UTC.
        let leap_second_input = if second == 60 {
            second = 59;
            nanosecond = 999_999_999;
            true
        } else {
            false
        };

        let date = Month::from_number(month)
            .and_then(|month| Date::from_calendar_date(year.cast_signed(), month, day))
            .map_err(TryFromParsed::ComponentRange)?;
        let time = Time::from_hms_nano(hour, minute, second, nanosecond)
            .map_err(TryFromParsed::ComponentRange)?;
        let dt = OffsetDateTime::new_in_offset(date, time, offset);

        if leap_second_input && !dt.is_valid_leap_second_stand_in() {
            return Err(error::Parse::TryFromParsed(TryFromParsed::ComponentRange(
                error::ComponentRange {
                    name: "second",
                    minimum: 0,
                    maximum: 59,
                    value: 60,
                    conditional_message: Some("because leap seconds are not supported"),
                },
            )));
        }

        Ok(dt)
    }
}

impl<const CONFIG: EncodedConfig> sealed::Sealed for Iso8601<CONFIG> {
    #[inline]
    fn parse_into<'a>(
        &self,
        mut input: &'a [u8],
        parsed: &mut Parsed,
    ) -> Result<&'a [u8], error::Parse> {
        use crate::parsing::combinator::rfc::iso8601::ExtendedKind;

        let mut extended_kind = ExtendedKind::Unknown;
        let mut date_is_present = false;
        let mut time_is_present = false;
        let mut offset_is_present = false;
        let mut first_error = None;

        parsed.leap_second_allowed = true;

        match Self::parse_date(parsed, &mut extended_kind)(input) {
            Ok(new_input) => {
                input = new_input;
                date_is_present = true;
            }
            Err(err) => {
                first_error.get_or_insert(err);
            }
        }

        match Self::parse_time(parsed, &mut extended_kind, date_is_present)(input) {
            Ok(new_input) => {
                input = new_input;
                time_is_present = true;
            }
            Err(err) => {
                first_error.get_or_insert(err);
            }
        }

        // If a date and offset are present, a time must be as well.
        if !date_is_present || time_is_present {
            match Self::parse_offset(parsed, &mut extended_kind)(input) {
                Ok(new_input) => {
                    input = new_input;
                    offset_is_present = true;
                }
                Err(err) => {
                    first_error.get_or_insert(err);
                }
            }
        }

        if !date_is_present && !time_is_present && !offset_is_present {
            match first_error {
                Some(err) => return Err(err),
                None => bug!("an error should be present if no components were parsed"),
            }
        }

        Ok(input)
    }
}
