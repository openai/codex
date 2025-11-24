use crate::{
    error::{err, ErrorContext},
    fmt::{
        friendly::parser_label,
        util::{
            fractional_time_to_duration, fractional_time_to_span,
            parse_temporal_fraction,
        },
        Parsed,
    },
    util::{
        escape,
        t::{self, C},
    },
    Error, SignedDuration, Span, Unit,
};

/// A parser for Jiff's "friendly" duration format.
///
/// See the [module documentation](super) for more details on the precise
/// format supported by this parser.
///
/// Unlike [`SpanPrinter`](super::SpanPrinter), this parser doesn't have any
/// configuration knobs. While it may grow some in the future, the approach
/// taken here is for the parser to support the entire grammar. That is, the
/// parser can parse anything emitted by `SpanPrinter`. (And indeed, the
/// parser can even handle things that the printer can't emit due to lack of
/// configurability. For example, `1hour1m` is a valid friendly duration,
/// but `SpanPrinter` cannot emit it due to a mixing of verbose and compact
/// designator labels.)
///
/// # Advice
///
/// Since this parser has no configuration, there are generally only two reasons
/// why you might want to use this type specifically:
///
/// 1. You need to parse from `&[u8]`.
/// 2. You need to parse _only_ the "friendly" format.
///
/// Otherwise, you can use the `FromStr` implementations on both `Span` and
/// `SignedDuration`, which automatically support the friendly format in
/// addition to the ISO 8601 format simultaneously:
///
/// ```
/// use jiff::{SignedDuration, Span, ToSpan};
///
/// let span: Span = "5 years, 2 months".parse()?;
/// assert_eq!(span, 5.years().months(2).fieldwise());
///
/// let sdur: SignedDuration = "5 hours, 2 minutes".parse()?;
/// assert_eq!(sdur, SignedDuration::new(5 * 60 * 60 + 2 * 60, 0));
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Example
///
/// This example shows how to parse a `Span` directly from `&str`:
///
/// ```
/// use jiff::{fmt::friendly::SpanParser, ToSpan};
///
/// static PARSER: SpanParser = SpanParser::new();
///
/// let string = "1 year, 3 months, 15:00:01.3";
/// let span = PARSER.parse_span(string)?;
/// assert_eq!(
///     span,
///     1.year().months(3).hours(15).seconds(1).milliseconds(300).fieldwise(),
/// );
///
/// // Negative durations are supported too!
/// let string = "1 year, 3 months, 15:00:01.3 ago";
/// let span = PARSER.parse_span(string)?;
/// assert_eq!(
///     span,
///     -1.year().months(3).hours(15).seconds(1).milliseconds(300).fieldwise(),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct SpanParser {
    _private: (),
}

impl SpanParser {
    /// Creates a new parser for the "friendly" duration format.
    ///
    /// The parser returned uses the default configuration. (Although, at time
    /// of writing, there are no available configuration options for this
    /// parser.) This is identical to `SpanParser::default`, but it can be used
    /// in a `const` context.
    ///
    /// # Example
    ///
    /// This example shows how to parse a `Span` directly from `&[u8]`:
    ///
    /// ```
    /// use jiff::{fmt::friendly::SpanParser, ToSpan};
    ///
    /// static PARSER: SpanParser = SpanParser::new();
    ///
    /// let bytes = b"1 year 3 months 15 hours 1300ms";
    /// let span = PARSER.parse_span(bytes)?;
    /// assert_eq!(
    ///     span,
    ///     1.year().months(3).hours(15).milliseconds(1300).fieldwise(),
    /// );
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[inline]
    pub const fn new() -> SpanParser {
        SpanParser { _private: () }
    }

    /// Run the parser on the given string (which may be plain bytes) and,
    /// if successful, return the parsed `Span`.
    ///
    /// See the [module documentation](super) for more details on the specific
    /// grammar supported by this parser.
    ///
    /// # Example
    ///
    /// This shows a number of different duration formats that can be parsed
    /// into a `Span`:
    ///
    /// ```
    /// use jiff::{fmt::friendly::SpanParser, ToSpan};
    ///
    /// let spans = [
    ///     ("40d", 40.days()),
    ///     ("40 days", 40.days()),
    ///     ("1y1d", 1.year().days(1)),
    ///     ("1yr 1d", 1.year().days(1)),
    ///     ("3d4h59m", 3.days().hours(4).minutes(59)),
    ///     ("3 days, 4 hours, 59 minutes", 3.days().hours(4).minutes(59)),
    ///     ("3d 4h 59m", 3.days().hours(4).minutes(59)),
    ///     ("2h30m", 2.hours().minutes(30)),
    ///     ("2h 30m", 2.hours().minutes(30)),
    ///     ("1mo", 1.month()),
    ///     ("1w", 1.week()),
    ///     ("1 week", 1.week()),
    ///     ("1w4d", 1.week().days(4)),
    ///     ("1 wk 4 days", 1.week().days(4)),
    ///     ("1m", 1.minute()),
    ///     ("0.0021s", 2.milliseconds().microseconds(100)),
    ///     ("0s", 0.seconds()),
    ///     ("0d", 0.seconds()),
    ///     ("0 days", 0.seconds()),
    ///     (
    ///         "1y1mo1d1h1m1.1s",
    ///         1.year().months(1).days(1).hours(1).minutes(1).seconds(1).milliseconds(100),
    ///     ),
    ///     (
    ///         "1yr 1mo 1day 1hr 1min 1.1sec",
    ///         1.year().months(1).days(1).hours(1).minutes(1).seconds(1).milliseconds(100),
    ///     ),
    ///     (
    ///         "1 year, 1 month, 1 day, 1 hour, 1 minute 1.1 seconds",
    ///         1.year().months(1).days(1).hours(1).minutes(1).seconds(1).milliseconds(100),
    ///     ),
    ///     (
    ///         "1 year, 1 month, 1 day, 01:01:01.1",
    ///         1.year().months(1).days(1).hours(1).minutes(1).seconds(1).milliseconds(100),
    ///     ),
    ///     (
    ///         "1 yr, 1 month, 1 d, 1 h, 1 min 1.1 second",
    ///         1.year().months(1).days(1).hours(1).minutes(1).seconds(1).milliseconds(100),
    ///     ),
    /// ];
    ///
    /// static PARSER: SpanParser = SpanParser::new();
    /// for (string, span) in spans {
    ///     let parsed = PARSER.parse_span(string)?;
    ///     assert_eq!(
    ///         span.fieldwise(),
    ///         parsed.fieldwise(),
    ///         "result of parsing {string:?}",
    ///     );
    /// }
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse_span<I: AsRef<[u8]>>(&self, input: I) -> Result<Span, Error> {
        let input = input.as_ref();
        let parsed = self.parse_to_span(input).with_context(|| {
            err!(
                "failed to parse {input:?} in the \"friendly\" format",
                input = escape::Bytes(input)
            )
        })?;
        let span = parsed.into_full().with_context(|| {
            err!(
                "failed to parse {input:?} in the \"friendly\" format",
                input = escape::Bytes(input)
            )
        })?;
        Ok(span)
    }

    /// Run the parser on the given string (which may be plain bytes) and,
    /// if successful, return the parsed `SignedDuration`.
    ///
    /// See the [module documentation](super) for more details on the specific
    /// grammar supported by this parser.
    ///
    /// # Example
    ///
    /// This shows a number of different duration formats that can be parsed
    /// into a `SignedDuration`:
    ///
    /// ```
    /// use jiff::{fmt::friendly::SpanParser, SignedDuration};
    ///
    /// let durations = [
    ///     ("2h30m", SignedDuration::from_secs(2 * 60 * 60 + 30 * 60)),
    ///     ("2 hrs 30 mins", SignedDuration::from_secs(2 * 60 * 60 + 30 * 60)),
    ///     ("2 hours 30 minutes", SignedDuration::from_secs(2 * 60 * 60 + 30 * 60)),
    ///     ("2 hrs 30 minutes", SignedDuration::from_secs(2 * 60 * 60 + 30 * 60)),
    ///     ("2.5h", SignedDuration::from_secs(2 * 60 * 60 + 30 * 60)),
    ///     ("1m", SignedDuration::from_mins(1)),
    ///     ("1.5m", SignedDuration::from_secs(90)),
    ///     ("0.0021s", SignedDuration::new(0, 2_100_000)),
    ///     ("0s", SignedDuration::ZERO),
    ///     ("0.000000001s", SignedDuration::from_nanos(1)),
    /// ];
    ///
    /// static PARSER: SpanParser = SpanParser::new();
    /// for (string, duration) in durations {
    ///     let parsed = PARSER.parse_duration(string)?;
    ///     assert_eq!(duration, parsed, "result of parsing {string:?}");
    /// }
    ///
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse_duration<I: AsRef<[u8]>>(
        &self,
        input: I,
    ) -> Result<SignedDuration, Error> {
        let input = input.as_ref();
        let parsed = self.parse_to_duration(input).with_context(|| {
            err!(
                "failed to parse {input:?} in the \"friendly\" format",
                input = escape::Bytes(input)
            )
        })?;
        let sdur = parsed.into_full().with_context(|| {
            err!(
                "failed to parse {input:?} in the \"friendly\" format",
                input = escape::Bytes(input)
            )
        })?;
        Ok(sdur)
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_to_span<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, Span>, Error> {
        if input.is_empty() {
            return Err(err!("an empty string is not a valid duration"));
        }
        // Guard prefix sign parsing to avoid the function call, which is
        // marked unlineable to keep the fast path tighter.
        let (sign, input) =
            if !input.first().map_or(false, |&b| matches!(b, b'+' | b'-')) {
                (None, input)
            } else {
                let Parsed { value: sign, input } =
                    self.parse_prefix_sign(input);
                (sign, input)
            };

        let Parsed { value, input } = self.parse_unit_value(input)?;
        let Some(first_unit_value) = value else {
            return Err(err!(
                "parsing a friendly duration requires it to start \
                 with a unit value (a decimal integer) after an \
                 optional sign, but no integer was found",
            ));
        };
        let Parsed { value: span, input } =
            self.parse_units_to_span(input, first_unit_value)?;

        // As with the prefix sign parsing, guard it to avoid calling the
        // function.
        let (sign, input) = if !input.first().map_or(false, is_whitespace) {
            (sign.unwrap_or(t::Sign::N::<1>()), input)
        } else {
            let parsed = self.parse_suffix_sign(sign, input)?;
            (parsed.value, parsed.input)
        };
        Ok(Parsed { value: span * i64::from(sign.get()), input })
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_to_duration<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, SignedDuration>, Error> {
        if input.is_empty() {
            return Err(err!("an empty string is not a valid duration"));
        }
        // Guard prefix sign parsing to avoid the function call, which is
        // marked unlineable to keep the fast path tighter.
        let (sign, input) =
            if !input.first().map_or(false, |&b| matches!(b, b'+' | b'-')) {
                (None, input)
            } else {
                let Parsed { value: sign, input } =
                    self.parse_prefix_sign(input);
                (sign, input)
            };

        let Parsed { value, input } = self.parse_unit_value(input)?;
        let Some(first_unit_value) = value else {
            return Err(err!(
                "parsing a friendly duration requires it to start \
                 with a unit value (a decimal integer) after an \
                 optional sign, but no integer was found",
            ));
        };
        let Parsed { value: mut sdur, input } =
            self.parse_units_to_duration(input, first_unit_value)?;

        // As with the prefix sign parsing, guard it to avoid calling the
        // function.
        let (sign, input) = if !input.first().map_or(false, is_whitespace) {
            (sign.unwrap_or(t::Sign::N::<1>()), input)
        } else {
            let parsed = self.parse_suffix_sign(sign, input)?;
            (parsed.value, parsed.input)
        };
        if sign < C(0) {
            sdur = -sdur;
        }

        Ok(Parsed { value: sdur, input })
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_units_to_span<'i>(
        &self,
        mut input: &'i [u8],
        first_unit_value: t::NoUnits,
    ) -> Result<Parsed<'i, Span>, Error> {
        let mut parsed_any_after_comma = true;
        let mut prev_unit: Option<Unit> = None;
        let mut value = first_unit_value;
        let mut span = Span::new();
        loop {
            let parsed = self.parse_hms_maybe(input, value)?;
            input = parsed.input;
            if let Some(hms) = parsed.value {
                if let Some(prev_unit) = prev_unit {
                    if prev_unit <= Unit::Hour {
                        return Err(err!(
                            "found 'HH:MM:SS' after unit {prev_unit}, \
                             but 'HH:MM:SS' can only appear after \
                             years, months, weeks or days",
                            prev_unit = prev_unit.singular(),
                        ));
                    }
                }
                span = set_span_unit_value(Unit::Hour, hms.hour, span)?;
                span = set_span_unit_value(Unit::Minute, hms.minute, span)?;
                span = if let Some(fraction) = hms.fraction {
                    fractional_time_to_span(
                        Unit::Second,
                        hms.second,
                        fraction,
                        span,
                    )?
                } else {
                    set_span_unit_value(Unit::Second, hms.second, span)?
                };
                break;
            }

            let fraction =
                if input.first().map_or(false, |&b| b == b'.' || b == b',') {
                    let parsed = parse_temporal_fraction(input)?;
                    input = parsed.input;
                    parsed.value
                } else {
                    None
                };

            // Eat any optional whitespace between the unit value and label.
            input = self.parse_optional_whitespace(input).input;

            // Parse the actual unit label/designator.
            let parsed = self.parse_unit_designator(input)?;
            input = parsed.input;
            let unit = parsed.value;

            // A comma is allowed to immediately follow the designator.
            // Since this is a rarer case, we guard it with a check to see
            // if the comma is there and only then call the function (which is
            // marked unlineable to try and keep the hot path tighter).
            if input.first().map_or(false, |&b| b == b',') {
                input = self.parse_optional_comma(input)?.input;
                parsed_any_after_comma = false;
            }

            if let Some(prev_unit) = prev_unit {
                if prev_unit <= unit {
                    return Err(err!(
                        "found value {value:?} with unit {unit} \
                         after unit {prev_unit}, but units must be \
                         written from largest to smallest \
                         (and they can't be repeated)",
                        unit = unit.singular(),
                        prev_unit = prev_unit.singular(),
                    ));
                }
            }
            prev_unit = Some(unit);

            if let Some(fraction) = fraction {
                span = fractional_time_to_span(unit, value, fraction, span)?;
                // Once we see a fraction, we are done. We don't permit parsing
                // any more units. That is, a fraction can only occur on the
                // lowest unit of time.
                break;
            } else {
                span = set_span_unit_value(unit, value, span)?;
            }

            // Eat any optional whitespace after the designator (or comma) and
            // before the next unit value. But if we don't see a unit value,
            // we don't eat the whitespace.
            let after_whitespace = self.parse_optional_whitespace(input).input;
            let parsed = self.parse_unit_value(after_whitespace)?;
            value = match parsed.value {
                None => break,
                Some(value) => value,
            };
            input = parsed.input;
            parsed_any_after_comma = true;
        }
        if !parsed_any_after_comma {
            return Err(err!(
                "found comma at the end of duration, \
                 but a comma indicates at least one more \
                 unit follows and none were found after \
                 {prev_unit}",
                // OK because parsed_any_after_comma can only
                // be false when prev_unit is set.
                prev_unit = prev_unit.unwrap().plural(),
            ));
        }
        Ok(Parsed { value: span, input })
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_units_to_duration<'i>(
        &self,
        mut input: &'i [u8],
        first_unit_value: t::NoUnits,
    ) -> Result<Parsed<'i, SignedDuration>, Error> {
        let mut parsed_any_after_comma = true;
        let mut prev_unit: Option<Unit> = None;
        let mut value = first_unit_value;
        let mut sdur = SignedDuration::ZERO;
        loop {
            let parsed = self.parse_hms_maybe(input, value)?;
            input = parsed.input;
            if let Some(hms) = parsed.value {
                if let Some(prev_unit) = prev_unit {
                    if prev_unit <= Unit::Hour {
                        return Err(err!(
                            "found 'HH:MM:SS' after unit {prev_unit}, \
                             but 'HH:MM:SS' can only appear after \
                             years, months, weeks or days",
                            prev_unit = prev_unit.singular(),
                        ));
                    }
                }
                sdur = sdur
                    .checked_add(duration_unit_value(Unit::Hour, hms.hour)?)
                    .ok_or_else(|| {
                        err!(
                            "accumulated `SignedDuration` overflowed when \
                             adding {value} of unit hour",
                        )
                    })?;
                sdur = sdur
                    .checked_add(duration_unit_value(
                        Unit::Minute,
                        hms.minute,
                    )?)
                    .ok_or_else(|| {
                        err!(
                            "accumulated `SignedDuration` overflowed when \
                             adding {value} of unit minute",
                        )
                    })?;
                sdur = sdur
                    .checked_add(duration_unit_value(
                        Unit::Second,
                        hms.second,
                    )?)
                    .ok_or_else(|| {
                        err!(
                            "accumulated `SignedDuration` overflowed when \
                             adding {value} of unit second",
                        )
                    })?;
                if let Some(f) = hms.fraction {
                    // nanos += fractional_time_to_nanos(Unit::Second, fraction)?;
                    let f = fractional_time_to_duration(Unit::Second, f)?;
                    sdur = sdur.checked_add(f).ok_or_else(|| err!(""))?;
                };
                break;
            }

            let fraction =
                if input.first().map_or(false, |&b| b == b'.' || b == b',') {
                    let parsed = parse_temporal_fraction(input)?;
                    input = parsed.input;
                    parsed.value
                } else {
                    None
                };

            // Eat any optional whitespace between the unit value and label.
            input = self.parse_optional_whitespace(input).input;

            // Parse the actual unit label/designator.
            let parsed = self.parse_unit_designator(input)?;
            input = parsed.input;
            let unit = parsed.value;

            // A comma is allowed to immediately follow the designator.
            // Since this is a rarer case, we guard it with a check to see
            // if the comma is there and only then call the function (which is
            // marked unlineable to try and keep the hot path tighter).
            if input.first().map_or(false, |&b| b == b',') {
                input = self.parse_optional_comma(input)?.input;
                parsed_any_after_comma = false;
            }

            if let Some(prev_unit) = prev_unit {
                if prev_unit <= unit {
                    return Err(err!(
                        "found value {value:?} with unit {unit} \
                         after unit {prev_unit}, but units must be \
                         written from largest to smallest \
                         (and they can't be repeated)",
                        unit = unit.singular(),
                        prev_unit = prev_unit.singular(),
                    ));
                }
            }
            prev_unit = Some(unit);

            sdur = sdur
                .checked_add(duration_unit_value(unit, value)?)
                .ok_or_else(|| {
                    err!(
                        "accumulated `SignedDuration` overflowed when adding \
                         {value} of unit {unit}",
                        unit = unit.singular(),
                    )
                })?;
            if let Some(f) = fraction {
                let f = fractional_time_to_duration(unit, f)?;
                sdur = sdur.checked_add(f).ok_or_else(|| err!(""))?;
                // Once we see a fraction, we are done. We don't permit parsing
                // any more units. That is, a fraction can only occur on the
                // lowest unit of time.
                break;
            }

            // Eat any optional whitespace after the designator (or comma) and
            // before the next unit value. But if we don't see a unit value,
            // we don't eat the whitespace.
            let after_whitespace = self.parse_optional_whitespace(input).input;
            let parsed = self.parse_unit_value(after_whitespace)?;
            value = match parsed.value {
                None => break,
                Some(value) => value,
            };
            input = parsed.input;
            parsed_any_after_comma = true;
        }
        if !parsed_any_after_comma {
            return Err(err!(
                "found comma at the end of duration, \
                 but a comma indicates at least one more \
                 unit follows and none were found after \
                 {prev_unit}",
                // OK because parsed_any_after_comma can only
                // be false when prev_unit is set.
                prev_unit = prev_unit.unwrap().plural(),
            ));
        }
        Ok(Parsed { value: sdur, input })
    }

    /// This possibly parses a `HH:MM:SS[.fraction]`.
    ///
    /// This expects that a unit value has been parsed and looks for a `:`
    /// at `input[0]`. If `:` is found, then this proceeds to parse HMS.
    /// Otherwise, a `None` value is returned.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_hms_maybe<'i>(
        &self,
        input: &'i [u8],
        hour: t::NoUnits,
    ) -> Result<Parsed<'i, Option<HMS>>, Error> {
        if !input.first().map_or(false, |&b| b == b':') {
            return Ok(Parsed { input, value: None });
        }
        let Parsed { input, value } = self.parse_hms(&input[1..], hour)?;
        Ok(Parsed { input, value: Some(value) })
    }

    /// This parses a `HH:MM:SS[.fraction]` when it is known/expected to be
    /// present.
    ///
    /// This is also marked as non-inlined since we expect this to be a
    /// less common case. Where as `parse_hms_maybe` is called unconditionally
    /// to check to see if the HMS should be parsed.
    ///
    /// This assumes that the beginning of `input` immediately follows the
    /// first `:` in `HH:MM:SS[.fraction]`.
    #[inline(never)]
    fn parse_hms<'i>(
        &self,
        input: &'i [u8],
        hour: t::NoUnits,
    ) -> Result<Parsed<'i, HMS>, Error> {
        let Parsed { input, value } = self.parse_unit_value(input)?;
        let Some(minute) = value else {
            return Err(err!(
                "expected to parse minute in 'HH:MM:SS' format \
                 following parsed hour of {hour}",
            ));
        };
        if !input.first().map_or(false, |&b| b == b':') {
            return Err(err!(
                "when parsing 'HH:MM:SS' format, expected to \
                 see a ':' after the parsed minute of {minute}",
            ));
        }
        let input = &input[1..];
        let Parsed { input, value } = self.parse_unit_value(input)?;
        let Some(second) = value else {
            return Err(err!(
                "expected to parse second in 'HH:MM:SS' format \
                 following parsed minute of {minute}",
            ));
        };
        let (fraction, input) =
            if input.first().map_or(false, |&b| b == b'.' || b == b',') {
                let parsed = parse_temporal_fraction(input)?;
                (parsed.value, parsed.input)
            } else {
                (None, input)
            };
        let hms = HMS { hour, minute, second, fraction };
        Ok(Parsed { input, value: hms })
    }

    /// Parsed a unit value, i.e., an integer.
    ///
    /// If no digits (`[0-9]`) were found at the current position of the parser
    /// then `None` is returned. This means, for example, that parsing a
    /// duration should stop.
    ///
    /// Note that this is safe to call on untrusted input. It will not attempt
    /// to consume more input than could possibly fit into a parsed integer.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_unit_value<'i>(
        &self,
        mut input: &'i [u8],
    ) -> Result<Parsed<'i, Option<t::NoUnits>>, Error> {
        // Discovered via `i64::MAX.to_string().len()`.
        const MAX_I64_DIGITS: usize = 19;

        let mut digit_count = 0;
        let mut n: i64 = 0;
        while digit_count <= MAX_I64_DIGITS
            && input.get(digit_count).map_or(false, u8::is_ascii_digit)
        {
            let byte = input[digit_count];
            digit_count += 1;

            // This part is manually inlined from `util::parse::i64`.
            // Namely, `parse::i64` requires knowing all of the
            // digits up front. But we don't really know that here.
            // So as we parse the digits, we also accumulate them
            // into an integer. This avoids a second pass. (I guess
            // `util::parse::i64` could be better designed? Meh.)
            let digit = match byte.checked_sub(b'0') {
                None => {
                    return Err(err!(
                        "invalid digit, expected 0-9 but got {}",
                        escape::Byte(byte),
                    ));
                }
                Some(digit) if digit > 9 => {
                    return Err(err!(
                        "invalid digit, expected 0-9 but got {}",
                        escape::Byte(byte),
                    ))
                }
                Some(digit) => {
                    debug_assert!((0..=9).contains(&digit));
                    i64::from(digit)
                }
            };
            n = n
                .checked_mul(10)
                .and_then(|n| n.checked_add(digit))
                .ok_or_else(|| {
                    err!(
                        "number '{}' too big to parse into 64-bit integer",
                        escape::Bytes(&input[..digit_count]),
                    )
                })?;
        }
        if digit_count == 0 {
            return Ok(Parsed { value: None, input });
        }

        input = &input[digit_count..];
        // OK because t::NoUnits permits all possible i64 values.
        let value = t::NoUnits::new(n).unwrap();
        Ok(Parsed { value: Some(value), input })
    }

    /// Parse a unit designator, e.g., `years` or `nano`.
    ///
    /// If no designator could be found, including if the given `input` is
    /// empty, then this return an error.
    ///
    /// This does not attempt to handle leading or trailing whitespace.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_unit_designator<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, Unit>, Error> {
        let Some((unit, len)) = parser_label::find(input) else {
            if input.is_empty() {
                return Err(err!(
                    "expected to find unit designator suffix \
                     (e.g., 'years' or 'secs'), \
                     but found end of input",
                ));
            } else {
                return Err(err!(
                    "expected to find unit designator suffix \
                     (e.g., 'years' or 'secs'), \
                     but found input beginning with {found:?} instead",
                    found = escape::Bytes(&input[..input.len().min(20)]),
                ));
            }
        };
        Ok(Parsed { value: unit, input: &input[len..] })
    }

    /// Parses an optional prefix sign from the given input.
    ///
    /// A prefix sign is either a `+` or a `-`. If neither are found, then
    /// `None` is returned.
    #[inline(never)]
    fn parse_prefix_sign<'i>(
        &self,
        input: &'i [u8],
    ) -> Parsed<'i, Option<t::Sign>> {
        let Some(sign) = input.first().copied() else {
            return Parsed { value: None, input };
        };
        let sign = if sign == b'+' {
            t::Sign::N::<1>()
        } else if sign == b'-' {
            t::Sign::N::<-1>()
        } else {
            return Parsed { value: None, input };
        };
        Parsed { value: Some(sign), input: &input[1..] }
    }

    /// Parses an optional suffix sign from the given input.
    ///
    /// This requires, as input, the result of parsing a prefix sign since this
    /// will return an error if both a prefix and a suffix sign were found.
    ///
    /// A suffix sign is the string `ago`. Any other string means that there is
    /// no suffix sign. This will also look for mandatory whitespace and eat
    /// any additional optional whitespace. i.e., This should be called
    /// immediately after parsing the last unit designator/label.
    ///
    /// Regardless of whether a prefix or suffix sign was found, a definitive
    /// sign is returned. (When there's no prefix or suffix sign, then the sign
    /// returned is positive.)
    #[inline(never)]
    fn parse_suffix_sign<'i>(
        &self,
        prefix_sign: Option<t::Sign>,
        mut input: &'i [u8],
    ) -> Result<Parsed<'i, t::Sign>, Error> {
        if !input.first().map_or(false, is_whitespace) {
            let sign = prefix_sign.unwrap_or(t::Sign::N::<1>());
            return Ok(Parsed { value: sign, input });
        }
        // Eat any additional whitespace we find before looking for 'ago'.
        input = self.parse_optional_whitespace(&input[1..]).input;
        let (suffix_sign, input) = if input.starts_with(b"ago") {
            (Some(t::Sign::N::<-1>()), &input[3..])
        } else {
            (None, input)
        };
        let sign = match (prefix_sign, suffix_sign) {
            (Some(_), Some(_)) => {
                return Err(err!(
                    "expected to find either a prefix sign (+/-) or \
                     a suffix sign (ago), but found both",
                ))
            }
            (Some(sign), None) => sign,
            (None, Some(sign)) => sign,
            (None, None) => t::Sign::N::<1>(),
        };
        Ok(Parsed { value: sign, input })
    }

    /// Parses an optional comma following a unit designator.
    ///
    /// If a comma is seen, then it is mandatory that it be followed by
    /// whitespace.
    ///
    /// This also takes care to provide a custom error message if the end of
    /// input is seen after a comma.
    ///
    /// If `input` doesn't start with a comma, then this is a no-op.
    #[inline(never)]
    fn parse_optional_comma<'i>(
        &self,
        mut input: &'i [u8],
    ) -> Result<Parsed<'i, ()>, Error> {
        if !input.first().map_or(false, |&b| b == b',') {
            return Ok(Parsed { value: (), input });
        }
        input = &input[1..];
        if input.is_empty() {
            return Err(err!(
                "expected whitespace after comma, but found end of input"
            ));
        }
        if !is_whitespace(&input[0]) {
            return Err(err!(
                "expected whitespace after comma, but found {found:?}",
                found = escape::Byte(input[0]),
            ));
        }
        Ok(Parsed { value: (), input: &input[1..] })
    }

    /// Parses zero or more bytes of ASCII whitespace.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_optional_whitespace<'i>(
        &self,
        mut input: &'i [u8],
    ) -> Parsed<'i, ()> {
        while input.first().map_or(false, is_whitespace) {
            input = &input[1..];
        }
        Parsed { value: (), input }
    }
}

/// A type that represents the parsed components of `HH:MM:SS[.fraction]`.
#[derive(Debug)]
struct HMS {
    hour: t::NoUnits,
    minute: t::NoUnits,
    second: t::NoUnits,
    fraction: Option<t::SubsecNanosecond>,
}

/// Set the given unit to the given value on the given span.
///
/// If the value outside the legal boundaries for the given unit, then an error
/// is returned.
#[cfg_attr(feature = "perf-inline", inline(always))]
fn set_span_unit_value(
    unit: Unit,
    value: t::NoUnits,
    mut span: Span,
) -> Result<Span, Error> {
    if unit <= Unit::Hour {
        let result = span.try_units_ranged(unit, value).with_context(|| {
            err!(
                "failed to set value {value:?} \
                 as {unit} unit on span",
                unit = Unit::from(unit).singular(),
            )
        });
        // This is annoying, but because we can write out a larger
        // number of hours/minutes/seconds than what we actually
        // support, we need to be prepared to parse an unbalanced span
        // if our time units are too big here.
        span = match result {
            Ok(span) => span,
            Err(_) => fractional_time_to_span(
                unit,
                value,
                t::SubsecNanosecond::N::<0>(),
                span,
            )?,
        };
    } else {
        span = span.try_units_ranged(unit, value).with_context(|| {
            err!(
                "failed to set value {value:?} \
                 as {unit} unit on span",
                unit = Unit::from(unit).singular(),
            )
        })?;
    }
    Ok(span)
}

/// Returns the given parsed value, interpreted as the given unit, as a
/// `SignedDuration`.
///
/// If the given unit is not supported for signed durations (i.e., calendar
/// units), or if converting the given value to a `SignedDuration` for the
/// given units overflows, then an error is returned.
#[cfg_attr(feature = "perf-inline", inline(always))]
fn duration_unit_value(
    unit: Unit,
    value: t::NoUnits,
) -> Result<SignedDuration, Error> {
    // let value = t::NoUnits128::rfrom(value);
    // Convert our parsed unit into a number of nanoseconds.
    //
    // Note also that overflow isn't possible here, since all of our parsed
    // values are guaranteed to fit into i64, but we accrue into an i128.
    // Of course, the final i128 might overflow a SignedDuration, but this
    // is checked once at the end of parsing when a SignedDuration is
    // materialized.
    let sdur = match unit {
        Unit::Hour => {
            let seconds =
                value.checked_mul(t::SECONDS_PER_HOUR).ok_or_else(|| {
                    err!("converting {value} hours to seconds overflows i64")
                })?;
            SignedDuration::from_secs(seconds.get())
        }
        Unit::Minute => {
            let seconds = value.try_checked_mul(
                "minutes-to-seconds",
                t::SECONDS_PER_MINUTE,
            )?;
            SignedDuration::from_secs(seconds.get())
        }
        Unit::Second => SignedDuration::from_secs(value.get()),
        Unit::Millisecond => SignedDuration::from_millis(value.get()),
        Unit::Microsecond => SignedDuration::from_micros(value.get()),
        Unit::Nanosecond => SignedDuration::from_nanos(value.get()),
        unsupported => {
            return Err(err!(
                "parsing {unit} units into a `SignedDuration` is not supported \
                 (perhaps try parsing into a `Span` instead)",
                unit = unsupported.singular(),
            ));
        }
    };
    Ok(sdur)
}

/// Returns true if the byte is ASCII whitespace.
#[cfg_attr(feature = "perf-inline", inline(always))]
fn is_whitespace(byte: &u8) -> bool {
    matches!(*byte, b' ' | b'\t' | b'\n' | b'\r' | b'\x0C')
}

#[cfg(feature = "alloc")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_span_basic() {
        let p = |s: &str| SpanParser::new().parse_span(s).unwrap();

        insta::assert_snapshot!(p("5 years"), @"P5Y");
        insta::assert_snapshot!(p("5 years 4 months"), @"P5Y4M");
        insta::assert_snapshot!(p("5 years 4 months 3 hours"), @"P5Y4MT3H");
        insta::assert_snapshot!(p("5 years, 4 months, 3 hours"), @"P5Y4MT3H");

        insta::assert_snapshot!(p("01:02:03"), @"PT1H2M3S");
        insta::assert_snapshot!(p("5 days 01:02:03"), @"P5DT1H2M3S");
        // This is Python's `str(timedelta)` format!
        insta::assert_snapshot!(p("5 days, 01:02:03"), @"P5DT1H2M3S");
        insta::assert_snapshot!(p("3yrs 5 days 01:02:03"), @"P3Y5DT1H2M3S");
        insta::assert_snapshot!(p("3yrs 5 days, 01:02:03"), @"P3Y5DT1H2M3S");
        insta::assert_snapshot!(
            p("3yrs 5 days, 01:02:03.123456789"),
            @"P3Y5DT1H2M3.123456789S",
        );
        insta::assert_snapshot!(p("999:999:999"), @"PT999H999M999S");
    }

    #[test]
    fn parse_span_fractional() {
        let p = |s: &str| SpanParser::new().parse_span(s).unwrap();

        insta::assert_snapshot!(p("1.5hrs"), @"PT1H30M");
        insta::assert_snapshot!(p("1.5mins"), @"PT1M30S");
        insta::assert_snapshot!(p("1.5secs"), @"PT1.5S");
        insta::assert_snapshot!(p("1.5msecs"), @"PT0.0015S");
        insta::assert_snapshot!(p("1.5µsecs"), @"PT0.0000015S");

        insta::assert_snapshot!(p("1d 1.5hrs"), @"P1DT1H30M");
        insta::assert_snapshot!(p("1h 1.5mins"), @"PT1H1M30S");
        insta::assert_snapshot!(p("1m 1.5secs"), @"PT1M1.5S");
        insta::assert_snapshot!(p("1s 1.5msecs"), @"PT1.0015S");
        insta::assert_snapshot!(p("1ms 1.5µsecs"), @"PT0.0010015S");

        insta::assert_snapshot!(p("1s2000ms"), @"PT3S");
    }

    #[test]
    fn parse_span_boundaries() {
        let p = |s: &str| SpanParser::new().parse_span(s).unwrap();

        insta::assert_snapshot!(p("19998 years"), @"P19998Y");
        insta::assert_snapshot!(p("19998 years ago"), @"-P19998Y");
        insta::assert_snapshot!(p("239976 months"), @"P239976M");
        insta::assert_snapshot!(p("239976 months ago"), @"-P239976M");
        insta::assert_snapshot!(p("1043497 weeks"), @"P1043497W");
        insta::assert_snapshot!(p("1043497 weeks ago"), @"-P1043497W");
        insta::assert_snapshot!(p("7304484 days"), @"P7304484D");
        insta::assert_snapshot!(p("7304484 days ago"), @"-P7304484D");
        insta::assert_snapshot!(p("175307616 hours"), @"PT175307616H");
        insta::assert_snapshot!(p("175307616 hours ago"), @"-PT175307616H");
        insta::assert_snapshot!(p("10518456960 minutes"), @"PT10518456960M");
        insta::assert_snapshot!(p("10518456960 minutes ago"), @"-PT10518456960M");
        insta::assert_snapshot!(p("631107417600 seconds"), @"PT631107417600S");
        insta::assert_snapshot!(p("631107417600 seconds ago"), @"-PT631107417600S");
        insta::assert_snapshot!(p("631107417600000 milliseconds"), @"PT631107417600S");
        insta::assert_snapshot!(p("631107417600000 milliseconds ago"), @"-PT631107417600S");
        insta::assert_snapshot!(p("631107417600000000 microseconds"), @"PT631107417600S");
        insta::assert_snapshot!(p("631107417600000000 microseconds ago"), @"-PT631107417600S");
        insta::assert_snapshot!(p("9223372036854775807 nanoseconds"), @"PT9223372036.854775807S");
        insta::assert_snapshot!(p("9223372036854775807 nanoseconds ago"), @"-PT9223372036.854775807S");

        insta::assert_snapshot!(p("175307617 hours"), @"PT175307616H60M");
        insta::assert_snapshot!(p("175307617 hours ago"), @"-PT175307616H60M");
        insta::assert_snapshot!(p("10518456961 minutes"), @"PT10518456960M60S");
        insta::assert_snapshot!(p("10518456961 minutes ago"), @"-PT10518456960M60S");
        insta::assert_snapshot!(p("631107417601 seconds"), @"PT631107417601S");
        insta::assert_snapshot!(p("631107417601 seconds ago"), @"-PT631107417601S");
        insta::assert_snapshot!(p("631107417600001 milliseconds"), @"PT631107417600.001S");
        insta::assert_snapshot!(p("631107417600001 milliseconds ago"), @"-PT631107417600.001S");
        insta::assert_snapshot!(p("631107417600000001 microseconds"), @"PT631107417600.000001S");
        insta::assert_snapshot!(p("631107417600000001 microseconds ago"), @"-PT631107417600.000001S");
        // We don't include nanoseconds here, because that will fail to
        // parse due to overflowing i64.
    }

    #[test]
    fn err_span_basic() {
        let p = |s: &str| SpanParser::new().parse_span(s).unwrap_err();

        insta::assert_snapshot!(
            p(""),
            @r###"failed to parse "" in the "friendly" format: an empty string is not a valid duration"###,
        );
        insta::assert_snapshot!(
            p(" "),
            @r###"failed to parse " " in the "friendly" format: parsing a friendly duration requires it to start with a unit value (a decimal integer) after an optional sign, but no integer was found"###,
        );
        insta::assert_snapshot!(
            p("a"),
            @r###"failed to parse "a" in the "friendly" format: parsing a friendly duration requires it to start with a unit value (a decimal integer) after an optional sign, but no integer was found"###,
        );
        insta::assert_snapshot!(
            p("2 months 1 year"),
            @r###"failed to parse "2 months 1 year" in the "friendly" format: found value 1 with unit year after unit month, but units must be written from largest to smallest (and they can't be repeated)"###,
        );
        insta::assert_snapshot!(
            p("1 year 1 mont"),
            @r###"failed to parse "1 year 1 mont" in the "friendly" format: parsed value 'P1Y1M', but unparsed input "nt" remains (expected no unparsed input)"###,
        );
        insta::assert_snapshot!(
            p("2 months,"),
            @r###"failed to parse "2 months," in the "friendly" format: expected whitespace after comma, but found end of input"###,
        );
        insta::assert_snapshot!(
            p("2 months, "),
            @r###"failed to parse "2 months, " in the "friendly" format: found comma at the end of duration, but a comma indicates at least one more unit follows and none were found after months"###,
        );
        insta::assert_snapshot!(
            p("2 months ,"),
            @r###"failed to parse "2 months ," in the "friendly" format: parsed value 'P2M', but unparsed input "," remains (expected no unparsed input)"###,
        );
    }

    #[test]
    fn err_span_sign() {
        let p = |s: &str| SpanParser::new().parse_span(s).unwrap_err();

        insta::assert_snapshot!(
            p("1yago"),
            @r###"failed to parse "1yago" in the "friendly" format: parsed value 'P1Y', but unparsed input "ago" remains (expected no unparsed input)"###,
        );
        insta::assert_snapshot!(
            p("1 year 1 monthago"),
            @r###"failed to parse "1 year 1 monthago" in the "friendly" format: parsed value 'P1Y1M', but unparsed input "ago" remains (expected no unparsed input)"###,
        );
        insta::assert_snapshot!(
            p("+1 year 1 month ago"),
            @r###"failed to parse "+1 year 1 month ago" in the "friendly" format: expected to find either a prefix sign (+/-) or a suffix sign (ago), but found both"###,
        );
        insta::assert_snapshot!(
            p("-1 year 1 month ago"),
            @r###"failed to parse "-1 year 1 month ago" in the "friendly" format: expected to find either a prefix sign (+/-) or a suffix sign (ago), but found both"###,
        );
    }

    #[test]
    fn err_span_overflow_fraction() {
        let p = |s: &str| SpanParser::new().parse_span(s).unwrap();
        let pe = |s: &str| SpanParser::new().parse_span(s).unwrap_err();

        insta::assert_snapshot!(
            // One fewer micro, and this parses okay. The error occurs because
            // the maximum number of microseconds is subtracted off, and we're
            // left over with a value that overflows an i64.
            pe("640330789636854776 micros"),
            @r###"failed to parse "640330789636854776 micros" in the "friendly" format: failed to set nanosecond value 9223372036854776000 on span determined from 640330789636854776.0: parameter 'nanoseconds' with value 9223372036854776000 is not in the required range of -9223372036854775807..=9223372036854775807"###,
        );
        // one fewer is okay
        insta::assert_snapshot!(
            p("640330789636854775 micros"),
            @"PT640330789636.854775S"
        );

        insta::assert_snapshot!(
            // This is like the test above, but actually exercises a slightly
            // different error path by using an explicit fraction. Here, if
            // we had x.807 micros, it would parse successfully.
            pe("640330789636854775.808 micros"),
            @r###"failed to parse "640330789636854775.808 micros" in the "friendly" format: failed to set nanosecond value 9223372036854775808 on span determined from 640330789636854775.808000000: parameter 'nanoseconds' with value 9223372036854775808 is not in the required range of -9223372036854775807..=9223372036854775807"###,
        );
        // one fewer is okay
        insta::assert_snapshot!(
            p("640330789636854775.807 micros"),
            @"PT640330789636.854775807S"
        );
    }

    #[test]
    fn err_span_overflow_units() {
        let p = |s: &str| SpanParser::new().parse_span(s).unwrap_err();

        insta::assert_snapshot!(
            p("19999 years"),
            @r###"failed to parse "19999 years" in the "friendly" format: failed to set value 19999 as year unit on span: parameter 'years' with value 19999 is not in the required range of -19998..=19998"###,
        );
        insta::assert_snapshot!(
            p("19999 years ago"),
            @r###"failed to parse "19999 years ago" in the "friendly" format: failed to set value 19999 as year unit on span: parameter 'years' with value 19999 is not in the required range of -19998..=19998"###,
        );

        insta::assert_snapshot!(
            p("239977 months"),
            @r###"failed to parse "239977 months" in the "friendly" format: failed to set value 239977 as month unit on span: parameter 'months' with value 239977 is not in the required range of -239976..=239976"###,
        );
        insta::assert_snapshot!(
            p("239977 months ago"),
            @r###"failed to parse "239977 months ago" in the "friendly" format: failed to set value 239977 as month unit on span: parameter 'months' with value 239977 is not in the required range of -239976..=239976"###,
        );

        insta::assert_snapshot!(
            p("1043498 weeks"),
            @r###"failed to parse "1043498 weeks" in the "friendly" format: failed to set value 1043498 as week unit on span: parameter 'weeks' with value 1043498 is not in the required range of -1043497..=1043497"###,
        );
        insta::assert_snapshot!(
            p("1043498 weeks ago"),
            @r###"failed to parse "1043498 weeks ago" in the "friendly" format: failed to set value 1043498 as week unit on span: parameter 'weeks' with value 1043498 is not in the required range of -1043497..=1043497"###,
        );

        insta::assert_snapshot!(
            p("7304485 days"),
            @r###"failed to parse "7304485 days" in the "friendly" format: failed to set value 7304485 as day unit on span: parameter 'days' with value 7304485 is not in the required range of -7304484..=7304484"###,
        );
        insta::assert_snapshot!(
            p("7304485 days ago"),
            @r###"failed to parse "7304485 days ago" in the "friendly" format: failed to set value 7304485 as day unit on span: parameter 'days' with value 7304485 is not in the required range of -7304484..=7304484"###,
        );

        insta::assert_snapshot!(
            p("9223372036854775808 nanoseconds"),
            @r###"failed to parse "9223372036854775808 nanoseconds" in the "friendly" format: number '9223372036854775808' too big to parse into 64-bit integer"###,
        );
        insta::assert_snapshot!(
            p("9223372036854775808 nanoseconds ago"),
            @r###"failed to parse "9223372036854775808 nanoseconds ago" in the "friendly" format: number '9223372036854775808' too big to parse into 64-bit integer"###,
        );
    }

    #[test]
    fn err_span_fraction() {
        let p = |s: &str| SpanParser::new().parse_span(s).unwrap_err();

        insta::assert_snapshot!(
            p("1.5 years"),
            @r###"failed to parse "1.5 years" in the "friendly" format: fractional year units are not allowed"###,
        );
        insta::assert_snapshot!(
            p("1.5 nanos"),
            @r###"failed to parse "1.5 nanos" in the "friendly" format: fractional nanosecond units are not allowed"###,
        );
    }

    #[test]
    fn err_span_hms() {
        let p = |s: &str| SpanParser::new().parse_span(s).unwrap_err();

        insta::assert_snapshot!(
            p("05:"),
            @r###"failed to parse "05:" in the "friendly" format: expected to parse minute in 'HH:MM:SS' format following parsed hour of 5"###,
        );
        insta::assert_snapshot!(
            p("05:06"),
            @r###"failed to parse "05:06" in the "friendly" format: when parsing 'HH:MM:SS' format, expected to see a ':' after the parsed minute of 6"###,
        );
        insta::assert_snapshot!(
            p("05:06:"),
            @r###"failed to parse "05:06:" in the "friendly" format: expected to parse second in 'HH:MM:SS' format following parsed minute of 6"###,
        );
        insta::assert_snapshot!(
            p("2 hours, 05:06:07"),
            @r###"failed to parse "2 hours, 05:06:07" in the "friendly" format: found 'HH:MM:SS' after unit hour, but 'HH:MM:SS' can only appear after years, months, weeks or days"###,
        );
    }

    #[test]
    fn parse_duration_basic() {
        let p = |s: &str| SpanParser::new().parse_duration(s).unwrap();

        insta::assert_snapshot!(p("1 hour, 2 minutes, 3 seconds"), @"PT1H2M3S");
        insta::assert_snapshot!(p("01:02:03"), @"PT1H2M3S");
        insta::assert_snapshot!(p("999:999:999"), @"PT1015H55M39S");
    }

    #[test]
    fn parse_duration_negate() {
        let p = |s: &str| SpanParser::new().parse_duration(s).unwrap();
        let perr = |s: &str| SpanParser::new().parse_duration(s).unwrap_err();

        insta::assert_snapshot!(
            p("9223372036854775807s"),
            @"PT2562047788015215H30M7S",
        );
        insta::assert_snapshot!(
            perr("9223372036854775808s"),
            @r###"failed to parse "9223372036854775808s" in the "friendly" format: number '9223372036854775808' too big to parse into 64-bit integer"###,
        );
        // This is kinda bush league, since -9223372036854775808 is the
        // minimum i64 value. But we fail to parse it because its absolute
        // value does not fit into an i64. Normally this would be bad juju
        // because it could imply that `SignedDuration::MIN` could serialize
        // successfully but then fail to deserialize. But the friendly printer
        // will try to use larger units before going to smaller units. So
        // `-9223372036854775808s` will never actually be emitted by the
        // friendly printer.
        insta::assert_snapshot!(
            perr("-9223372036854775808s"),
            @r###"failed to parse "-9223372036854775808s" in the "friendly" format: number '9223372036854775808' too big to parse into 64-bit integer"###,
        );
    }

    #[test]
    fn parse_duration_fractional() {
        let p = |s: &str| SpanParser::new().parse_duration(s).unwrap();

        insta::assert_snapshot!(p("1.5hrs"), @"PT1H30M");
        insta::assert_snapshot!(p("1.5mins"), @"PT1M30S");
        insta::assert_snapshot!(p("1.5secs"), @"PT1.5S");
        insta::assert_snapshot!(p("1.5msecs"), @"PT0.0015S");
        insta::assert_snapshot!(p("1.5µsecs"), @"PT0.0000015S");

        insta::assert_snapshot!(p("1h 1.5mins"), @"PT1H1M30S");
        insta::assert_snapshot!(p("1m 1.5secs"), @"PT1M1.5S");
        insta::assert_snapshot!(p("1s 1.5msecs"), @"PT1.0015S");
        insta::assert_snapshot!(p("1ms 1.5µsecs"), @"PT0.0010015S");

        insta::assert_snapshot!(p("1s2000ms"), @"PT3S");
    }

    #[test]
    fn parse_duration_boundaries() {
        let p = |s: &str| SpanParser::new().parse_duration(s).unwrap();
        let pe = |s: &str| SpanParser::new().parse_duration(s).unwrap_err();

        insta::assert_snapshot!(p("175307616 hours"), @"PT175307616H");
        insta::assert_snapshot!(p("175307616 hours ago"), @"-PT175307616H");
        insta::assert_snapshot!(p("10518456960 minutes"), @"PT175307616H");
        insta::assert_snapshot!(p("10518456960 minutes ago"), @"-PT175307616H");
        insta::assert_snapshot!(p("631107417600 seconds"), @"PT175307616H");
        insta::assert_snapshot!(p("631107417600 seconds ago"), @"-PT175307616H");
        insta::assert_snapshot!(p("631107417600000 milliseconds"), @"PT175307616H");
        insta::assert_snapshot!(p("631107417600000 milliseconds ago"), @"-PT175307616H");
        insta::assert_snapshot!(p("631107417600000000 microseconds"), @"PT175307616H");
        insta::assert_snapshot!(p("631107417600000000 microseconds ago"), @"-PT175307616H");
        insta::assert_snapshot!(p("9223372036854775807 nanoseconds"), @"PT2562047H47M16.854775807S");
        insta::assert_snapshot!(p("9223372036854775807 nanoseconds ago"), @"-PT2562047H47M16.854775807S");

        insta::assert_snapshot!(p("175307617 hours"), @"PT175307617H");
        insta::assert_snapshot!(p("175307617 hours ago"), @"-PT175307617H");
        insta::assert_snapshot!(p("10518456961 minutes"), @"PT175307616H1M");
        insta::assert_snapshot!(p("10518456961 minutes ago"), @"-PT175307616H1M");
        insta::assert_snapshot!(p("631107417601 seconds"), @"PT175307616H1S");
        insta::assert_snapshot!(p("631107417601 seconds ago"), @"-PT175307616H1S");
        insta::assert_snapshot!(p("631107417600001 milliseconds"), @"PT175307616H0.001S");
        insta::assert_snapshot!(p("631107417600001 milliseconds ago"), @"-PT175307616H0.001S");
        insta::assert_snapshot!(p("631107417600000001 microseconds"), @"PT175307616H0.000001S");
        insta::assert_snapshot!(p("631107417600000001 microseconds ago"), @"-PT175307616H0.000001S");
        // We don't include nanoseconds here, because that will fail to
        // parse due to overflowing i64.

        // The above were copied from the corresponding `Span` test, which has
        // tighter limits on components. But a `SignedDuration` supports the
        // full range of `i64` seconds.
        insta::assert_snapshot!(p("2562047788015215hours"), @"PT2562047788015215H");
        insta::assert_snapshot!(p("-2562047788015215hours"), @"-PT2562047788015215H");
        insta::assert_snapshot!(
            pe("2562047788015216hrs"),
            @r###"failed to parse "2562047788015216hrs" in the "friendly" format: converting 2562047788015216 hours to seconds overflows i64"###,
        );

        insta::assert_snapshot!(p("153722867280912930minutes"), @"PT2562047788015215H30M");
        insta::assert_snapshot!(p("153722867280912930minutes ago"), @"-PT2562047788015215H30M");
        insta::assert_snapshot!(
            pe("153722867280912931mins"),
            @r###"failed to parse "153722867280912931mins" in the "friendly" format: parameter 'minutes-to-seconds' with value 60 is not in the required range of -9223372036854775808..=9223372036854775807"###,
        );

        insta::assert_snapshot!(p("9223372036854775807seconds"), @"PT2562047788015215H30M7S");
        insta::assert_snapshot!(p("-9223372036854775807seconds"), @"-PT2562047788015215H30M7S");
        insta::assert_snapshot!(
            pe("9223372036854775808s"),
            @r###"failed to parse "9223372036854775808s" in the "friendly" format: number '9223372036854775808' too big to parse into 64-bit integer"###,
        );
        insta::assert_snapshot!(
            pe("-9223372036854775808s"),
            @r###"failed to parse "-9223372036854775808s" in the "friendly" format: number '9223372036854775808' too big to parse into 64-bit integer"###,
        );
    }

    #[test]
    fn err_duration_basic() {
        let p = |s: &str| SpanParser::new().parse_duration(s).unwrap_err();

        insta::assert_snapshot!(
            p(""),
            @r###"failed to parse "" in the "friendly" format: an empty string is not a valid duration"###,
        );
        insta::assert_snapshot!(
            p(" "),
            @r###"failed to parse " " in the "friendly" format: parsing a friendly duration requires it to start with a unit value (a decimal integer) after an optional sign, but no integer was found"###,
        );
        insta::assert_snapshot!(
            p("5"),
            @r###"failed to parse "5" in the "friendly" format: expected to find unit designator suffix (e.g., 'years' or 'secs'), but found end of input"###,
        );
        insta::assert_snapshot!(
            p("a"),
            @r###"failed to parse "a" in the "friendly" format: parsing a friendly duration requires it to start with a unit value (a decimal integer) after an optional sign, but no integer was found"###,
        );
        insta::assert_snapshot!(
            p("2 minutes 1 hour"),
            @r###"failed to parse "2 minutes 1 hour" in the "friendly" format: found value 1 with unit hour after unit minute, but units must be written from largest to smallest (and they can't be repeated)"###,
        );
        insta::assert_snapshot!(
            p("1 hour 1 minut"),
            @r###"failed to parse "1 hour 1 minut" in the "friendly" format: parsed value 'PT1H1M', but unparsed input "ut" remains (expected no unparsed input)"###,
        );
        insta::assert_snapshot!(
            p("2 minutes,"),
            @r###"failed to parse "2 minutes," in the "friendly" format: expected whitespace after comma, but found end of input"###,
        );
        insta::assert_snapshot!(
            p("2 minutes, "),
            @r###"failed to parse "2 minutes, " in the "friendly" format: found comma at the end of duration, but a comma indicates at least one more unit follows and none were found after minutes"###,
        );
        insta::assert_snapshot!(
            p("2 minutes ,"),
            @r###"failed to parse "2 minutes ," in the "friendly" format: parsed value 'PT2M', but unparsed input "," remains (expected no unparsed input)"###,
        );
    }

    #[test]
    fn err_duration_sign() {
        let p = |s: &str| SpanParser::new().parse_duration(s).unwrap_err();

        insta::assert_snapshot!(
            p("1hago"),
            @r###"failed to parse "1hago" in the "friendly" format: parsed value 'PT1H', but unparsed input "ago" remains (expected no unparsed input)"###,
        );
        insta::assert_snapshot!(
            p("1 hour 1 minuteago"),
            @r###"failed to parse "1 hour 1 minuteago" in the "friendly" format: parsed value 'PT1H1M', but unparsed input "ago" remains (expected no unparsed input)"###,
        );
        insta::assert_snapshot!(
            p("+1 hour 1 minute ago"),
            @r###"failed to parse "+1 hour 1 minute ago" in the "friendly" format: expected to find either a prefix sign (+/-) or a suffix sign (ago), but found both"###,
        );
        insta::assert_snapshot!(
            p("-1 hour 1 minute ago"),
            @r###"failed to parse "-1 hour 1 minute ago" in the "friendly" format: expected to find either a prefix sign (+/-) or a suffix sign (ago), but found both"###,
        );
    }

    #[test]
    fn err_duration_overflow_fraction() {
        let p = |s: &str| SpanParser::new().parse_duration(s).unwrap();
        let pe = |s: &str| SpanParser::new().parse_duration(s).unwrap_err();

        insta::assert_snapshot!(
            // Unlike `Span`, this just overflows because it can't be parsed
            // as a 64-bit integer.
            pe("9223372036854775808 micros"),
            @r###"failed to parse "9223372036854775808 micros" in the "friendly" format: number '9223372036854775808' too big to parse into 64-bit integer"###,
        );
        // one fewer is okay
        insta::assert_snapshot!(
            p("9223372036854775807 micros"),
            @"PT2562047788H54.775807S"
        );
    }

    #[test]
    fn err_duration_fraction() {
        let p = |s: &str| SpanParser::new().parse_duration(s).unwrap_err();

        insta::assert_snapshot!(
            p("1.5 nanos"),
            @r###"failed to parse "1.5 nanos" in the "friendly" format: fractional nanosecond units are not allowed"###,
        );
    }

    #[test]
    fn err_duration_hms() {
        let p = |s: &str| SpanParser::new().parse_duration(s).unwrap_err();

        insta::assert_snapshot!(
            p("05:"),
            @r###"failed to parse "05:" in the "friendly" format: expected to parse minute in 'HH:MM:SS' format following parsed hour of 5"###,
        );
        insta::assert_snapshot!(
            p("05:06"),
            @r###"failed to parse "05:06" in the "friendly" format: when parsing 'HH:MM:SS' format, expected to see a ':' after the parsed minute of 6"###,
        );
        insta::assert_snapshot!(
            p("05:06:"),
            @r###"failed to parse "05:06:" in the "friendly" format: expected to parse second in 'HH:MM:SS' format following parsed minute of 6"###,
        );
        insta::assert_snapshot!(
            p("2 hours, 05:06:07"),
            @r###"failed to parse "2 hours, 05:06:07" in the "friendly" format: found 'HH:MM:SS' after unit hour, but 'HH:MM:SS' can only appear after years, months, weeks or days"###,
        );
    }
}
