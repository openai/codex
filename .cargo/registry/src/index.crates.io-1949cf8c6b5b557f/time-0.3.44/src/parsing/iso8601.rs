//! Parse parts of an ISO 8601-formatted value.

#[allow(unused_imports, reason = "MSRV of 1.87")]
use num_conv::prelude::*;

use crate::convert::*;
use crate::error;
use crate::error::ParseFromDescription::{InvalidComponent, InvalidLiteral};
use crate::format_description::well_known::iso8601::EncodedConfig;
use crate::format_description::well_known::Iso8601;
use crate::parsing::combinator::rfc::iso8601::{
    day, dayk, dayo, float, hour, min, month, week, year, ExtendedKind,
};
use crate::parsing::combinator::{ascii_char, sign};
use crate::parsing::{Parsed, ParsedItem};

impl<const CONFIG: EncodedConfig> Iso8601<CONFIG> {
    // Basic: [year][month][day]
    // Extended: [year]["-"][month]["-"][day]
    // Basic: [year][dayo]
    // Extended: [year]["-"][dayo]
    // Basic: [year]["W"][week][dayk]
    // Extended: [year]["-"]["W"][week]["-"][dayk]
    /// Parse a date in the basic or extended format. Reduced precision is permitted.
    pub(crate) fn parse_date<'a>(
        parsed: &'a mut Parsed,
        extended_kind: &'a mut ExtendedKind,
    ) -> impl FnMut(&[u8]) -> Result<&[u8], error::Parse> + 'a {
        move |input| {
            // Same for any acceptable format.
            let ParsedItem(mut input, year) = year(input).ok_or(InvalidComponent("year"))?;
            *extended_kind = match ascii_char::<b'-'>(input) {
                Some(ParsedItem(new_input, ())) => {
                    input = new_input;
                    ExtendedKind::Extended
                }
                None => ExtendedKind::Basic, // no separator before mandatory month/ordinal/week
            };

            let parsed_month_day = (|| {
                let ParsedItem(mut input, month) = month(input).ok_or(InvalidComponent("month"))?;
                if extended_kind.is_extended() {
                    input = ascii_char::<b'-'>(input)
                        .ok_or(InvalidLiteral)?
                        .into_inner();
                }
                let ParsedItem(input, day) = day(input).ok_or(InvalidComponent("day"))?;
                Ok(ParsedItem(input, (month, day)))
            })();
            let mut ret_error = match parsed_month_day {
                Ok(ParsedItem(input, (month, day))) => {
                    *parsed = parsed
                        .with_year(year)
                        .ok_or(InvalidComponent("year"))?
                        .with_month(month)
                        .ok_or(InvalidComponent("month"))?
                        .with_day(day)
                        .ok_or(InvalidComponent("day"))?;
                    return Ok(input);
                }
                Err(err) => err,
            };

            // Don't check for `None`, as the error from year-month-day will always take priority.
            if let Some(ParsedItem(input, ordinal)) = dayo(input) {
                *parsed = parsed
                    .with_year(year)
                    .ok_or(InvalidComponent("year"))?
                    .with_ordinal(ordinal)
                    .ok_or(InvalidComponent("ordinal"))?;
                return Ok(input);
            }

            let parsed_week_weekday = (|| {
                let input = ascii_char::<b'W'>(input)
                    .ok_or((false, InvalidLiteral))?
                    .into_inner();
                let ParsedItem(mut input, week) =
                    week(input).ok_or((true, InvalidComponent("week")))?;
                if extended_kind.is_extended() {
                    input = ascii_char::<b'-'>(input)
                        .ok_or((true, InvalidLiteral))?
                        .into_inner();
                }
                let ParsedItem(input, weekday) =
                    dayk(input).ok_or((true, InvalidComponent("weekday")))?;
                Ok(ParsedItem(input, (week, weekday)))
            })();
            match parsed_week_weekday {
                Ok(ParsedItem(input, (week, weekday))) => {
                    *parsed = parsed
                        .with_iso_year(year)
                        .ok_or(InvalidComponent("year"))?
                        .with_iso_week_number(week)
                        .ok_or(InvalidComponent("week"))?
                        .with_weekday(weekday)
                        .ok_or(InvalidComponent("weekday"))?;
                    return Ok(input);
                }
                Err((false, _err)) => {}
                // This error is more accurate than the one from year-month-day.
                Err((true, err)) => ret_error = err,
            }

            Err(ret_error.into())
        }
    }

    // Basic: ["T"][hour][min][sec]
    // Extended: ["T"][hour][":"][min][":"][sec]
    // Reduced precision: components after [hour] (including their preceding separator) can be
    // omitted. ["T"] can be omitted if there is no date present.
    /// Parse a time in the basic or extended format. Reduced precision is permitted.
    pub(crate) fn parse_time<'a>(
        parsed: &'a mut Parsed,
        extended_kind: &'a mut ExtendedKind,
        date_is_present: bool,
    ) -> impl FnMut(&[u8]) -> Result<&[u8], error::Parse> + 'a {
        move |mut input| {
            if date_is_present {
                input = ascii_char::<b'T'>(input)
                    .ok_or(InvalidLiteral)?
                    .into_inner();
            }

            let ParsedItem(mut input, hour) = float(input).ok_or(InvalidComponent("hour"))?;
            match hour {
                (hour, None) => parsed.set_hour_24(hour).ok_or(InvalidComponent("hour"))?,
                (hour, Some(fractional_part)) => {
                    *parsed = parsed
                        .with_hour_24(hour)
                        .ok_or(InvalidComponent("hour"))?
                        .with_minute((fractional_part * Second::per_t::<f64>(Minute)) as u8)
                        .ok_or(InvalidComponent("minute"))?
                        .with_second(
                            (fractional_part * Second::per_t::<f64>(Hour)
                                % Minute::per_t::<f64>(Hour)) as u8,
                        )
                        .ok_or(InvalidComponent("second"))?
                        .with_subsecond(
                            (fractional_part * Nanosecond::per_t::<f64>(Hour)
                                % Nanosecond::per_t::<f64>(Second))
                                as u32,
                        )
                        .ok_or(InvalidComponent("subsecond"))?;
                    return Ok(input);
                }
            };

            if let Some(ParsedItem(new_input, ())) = ascii_char::<b':'>(input) {
                extended_kind
                    .coerce_extended()
                    .ok_or(InvalidComponent("minute"))?;
                input = new_input;
            };

            let mut input = match float(input) {
                Some(ParsedItem(input, (minute, None))) => {
                    extended_kind.coerce_basic();
                    parsed
                        .set_minute(minute)
                        .ok_or(InvalidComponent("minute"))?;
                    input
                }
                Some(ParsedItem(input, (minute, Some(fractional_part)))) => {
                    // `None` is valid behavior, so don't error if this fails.
                    extended_kind.coerce_basic();
                    *parsed = parsed
                        .with_minute(minute)
                        .ok_or(InvalidComponent("minute"))?
                        .with_second((fractional_part * Second::per_t::<f64>(Minute)) as u8)
                        .ok_or(InvalidComponent("second"))?
                        .with_subsecond(
                            (fractional_part * Nanosecond::per_t::<f64>(Minute)
                                % Nanosecond::per_t::<f64>(Second))
                                as u32,
                        )
                        .ok_or(InvalidComponent("subsecond"))?;
                    return Ok(input);
                }
                // colon was present, so minutes are required
                None if extended_kind.is_extended() => {
                    return Err(error::Parse::ParseFromDescription(InvalidComponent(
                        "minute",
                    )));
                }
                None => {
                    // Missing components are assumed to be zero.
                    *parsed = parsed
                        .with_minute(0)
                        .ok_or(InvalidComponent("minute"))?
                        .with_second(0)
                        .ok_or(InvalidComponent("second"))?
                        .with_subsecond(0)
                        .ok_or(InvalidComponent("subsecond"))?;
                    return Ok(input);
                }
            };

            if extended_kind.is_extended() {
                match ascii_char::<b':'>(input) {
                    Some(ParsedItem(new_input, ())) => input = new_input,
                    None => {
                        *parsed = parsed
                            .with_second(0)
                            .ok_or(InvalidComponent("second"))?
                            .with_subsecond(0)
                            .ok_or(InvalidComponent("subsecond"))?;
                        return Ok(input);
                    }
                }
            }

            let (input, second, subsecond) = match float(input) {
                Some(ParsedItem(input, (second, None))) => (input, second, 0),
                Some(ParsedItem(input, (second, Some(fractional_part)))) => (
                    input,
                    second,
                    round(fractional_part * Nanosecond::per_t::<f64>(Second)) as u32,
                ),
                None if extended_kind.is_extended() => {
                    return Err(error::Parse::ParseFromDescription(InvalidComponent(
                        "second",
                    )));
                }
                // Missing components are assumed to be zero.
                None => (input, 0, 0),
            };
            *parsed = parsed
                .with_second(second)
                .ok_or(InvalidComponent("second"))?
                .with_subsecond(subsecond)
                .ok_or(InvalidComponent("subsecond"))?;

            Ok(input)
        }
    }

    // Basic: [±][hour][min] or ["Z"]
    // Extended: [±][hour][":"][min] or ["Z"]
    // Reduced precision: [±][hour] or ["Z"]
    /// Parse a UTC offset in the basic or extended format. Reduced precision is supported.
    pub(crate) fn parse_offset<'a>(
        parsed: &'a mut Parsed,
        extended_kind: &'a mut ExtendedKind,
    ) -> impl FnMut(&[u8]) -> Result<&[u8], error::Parse> + 'a {
        move |input| {
            if let Some(ParsedItem(input, ())) = ascii_char::<b'Z'>(input) {
                *parsed = parsed
                    .with_offset_hour(0)
                    .ok_or(InvalidComponent("offset hour"))?
                    .with_offset_minute_signed(0)
                    .ok_or(InvalidComponent("offset minute"))?
                    .with_offset_second_signed(0)
                    .ok_or(InvalidComponent("offset second"))?;
                return Ok(input);
            }

            let ParsedItem(input, sign) = sign(input).ok_or(InvalidComponent("offset hour"))?;
            let mut input = hour(input)
                .and_then(|parsed_item| {
                    parsed_item.consume_value(|hour| {
                        parsed.set_offset_hour(if sign == b'-' {
                            -hour.cast_signed()
                        } else {
                            hour.cast_signed()
                        })
                    })
                })
                .ok_or(InvalidComponent("offset hour"))?;

            if extended_kind.maybe_extended() {
                if let Some(ParsedItem(new_input, ())) = ascii_char::<b':'>(input) {
                    extended_kind
                        .coerce_extended()
                        .ok_or(InvalidComponent("offset minute"))?;
                    input = new_input;
                };
            }

            match min(input) {
                Some(ParsedItem(new_input, min)) => {
                    input = new_input;
                    parsed
                        .set_offset_minute_signed(if sign == b'-' {
                            -min.cast_signed()
                        } else {
                            min.cast_signed()
                        })
                        .ok_or(InvalidComponent("offset minute"))?;
                }
                None => {
                    // Omitted offset minute is assumed to be zero.
                    parsed.set_offset_minute_signed(0);
                }
            }

            // If `:` was present, the format has already been set to extended. As such, this call
            // will do nothing in that case. If there wasn't `:` but minutes were
            // present, we know it's the basic format. Do not use `?` on the call, as
            // returning `None` is valid behavior.
            extended_kind.coerce_basic();

            Ok(input)
        }
    }
}

/// Round wrapper that uses hardware implementation if `std` is available, falling back to manual
/// implementation for `no_std`
#[inline]
fn round(value: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        value.round()
    }
    #[cfg(not(feature = "std"))]
    {
        debug_assert!(value.is_sign_positive() && !value.is_nan());

        let f = value % 1.;
        if f < 0.5 {
            value - f
        } else {
            value - f + 1.
        }
    }
}
