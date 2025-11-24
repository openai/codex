/*!
This module provides facilities for parsing time zone offsets.

The parsing here follows primarily from [RFC 3339] and [ISO 8601], but also
from [Temporal's hybrid grammar].

[RFC 3339]: https://www.rfc-editor.org/rfc/rfc3339
[ISO 8601]: https://www.iso.org/iso-8601-date-and-time-format.html
[Temporal's hybrid grammar]: https://tc39.es/proposal-temporal/#sec-temporal-iso8601grammar
*/

// Here's the specific part of Temporal's grammar that is implemented below:
//
// DateTimeUTCOffset :::
//   UTCDesignator
//   UTCOffsetSubMinutePrecision
//
// TimeZoneUTCOffsetName :::
//   UTCOffsetMinutePrecision
//
// UTCDesignator ::: one of
//   Z z
//
// UTCOffsetSubMinutePrecision :::
//   UTCOffsetMinutePrecision
//   UTCOffsetWithSubMinuteComponents[+Extended]
//   UTCOffsetWithSubMinuteComponents[~Extended]
//
// UTCOffsetMinutePrecision :::
//   TemporalSign Hour
//   TemporalSign Hour TimeSeparator[+Extended] MinuteSecond
//   TemporalSign Hour TimeSeparator[~Extended] MinuteSecond
//
// UTCOffsetWithSubMinuteComponents[Extended] :::
//   TemporalSign Hour
//     TimeSeparator[?Extended] MinuteSecond
//     TimeSeparator[?Extended] MinuteSecond
//     TemporalDecimalFraction[opt]
//
// TimeSeparator[Extended] :::
//   [+Extended] :
//   [~Extended] [empty]
//
// TemporalSign :::
//   ASCIISign
//   <MINUS>
//
// ASCIISign ::: one of
//   + -
//
// Hour :::
//   0 DecimalDigit
//   1 DecimalDigit
//   20
//   21
//   22
//   23
//
// MinuteSecond :::
//   0 DecimalDigit
//   1 DecimalDigit
//   2 DecimalDigit
//   3 DecimalDigit
//   4 DecimalDigit
//   5 DecimalDigit
//
// DecimalDigit :: one of
//   0 1 2 3 4 5 6 7 8 9
//
// TemporalDecimalFraction :::
//   TemporalDecimalSeparator DecimalDigit
//   TemporalDecimalSeparator DecimalDigit DecimalDigit
//   TemporalDecimalSeparator DecimalDigit DecimalDigit DecimalDigit
//   TemporalDecimalSeparator DecimalDigit DecimalDigit DecimalDigit
//                            DecimalDigit
//   TemporalDecimalSeparator DecimalDigit DecimalDigit DecimalDigit
//                            DecimalDigit DecimalDigit
//   TemporalDecimalSeparator DecimalDigit DecimalDigit DecimalDigit
//                            DecimalDigit DecimalDigit DecimalDigit
//   TemporalDecimalSeparator DecimalDigit DecimalDigit DecimalDigit
//                            DecimalDigit DecimalDigit DecimalDigit
//                            DecimalDigit
//   TemporalDecimalSeparator DecimalDigit DecimalDigit DecimalDigit
//                            DecimalDigit DecimalDigit DecimalDigit
//                            DecimalDigit DecimalDigit
//   TemporalDecimalSeparator DecimalDigit DecimalDigit DecimalDigit
//                            DecimalDigit DecimalDigit DecimalDigit
//                            DecimalDigit DecimalDigit DecimalDigit
//   TemporalDecimalSeparator ::: one of
//   . ,
//
// The quick summary of the above is that offsets up to nanosecond precision
// are supported. The general format is `{+,-}HH[:MM[:SS[.NNNNNNNNN]]]`. But
// ISO 8601 extended or basic formats are also supported. For example, the
// basic format `-0530` is equivalent to the extended format `-05:30`.
//
// Note that even though we support parsing up to nanosecond precision, Jiff
// currently only supports offsets up to second precision. I don't think there
// is any real practical need for any greater precision, but I don't think it
// would be too hard to switch an `Offset` from an `i32` representation in
// seconds to a `i64` representation in nanoseconds. (Since it only needs to
// support a span of time of about 52 hours or so.)

use crate::{
    error::{err, Error, ErrorContext},
    fmt::{
        temporal::{PiecesNumericOffset, PiecesOffset},
        util::{parse_temporal_fraction, FractionalFormatter},
        Parsed,
    },
    tz::Offset,
    util::{
        escape, parse,
        rangeint::{ri8, RFrom},
        t::{self, C},
    },
};

// We define our own ranged types because we want them to only be positive. We
// represent the sign explicitly as a separate field. But the range supported
// is the same as the component fields of `Offset`.
type ParsedOffsetHours = ri8<0, { t::SpanZoneOffsetHours::MAX }>;
type ParsedOffsetMinutes = ri8<0, { t::SpanZoneOffsetMinutes::MAX }>;
type ParsedOffsetSeconds = ri8<0, { t::SpanZoneOffsetSeconds::MAX }>;

/// An offset that has been parsed from a datetime string.
///
/// This represents either a Zulu offset (corresponding to UTC with an unknown
/// time zone offset), or a specific numeric offset given in hours, minutes,
/// seconds and nanoseconds (with everything except hours being optional).
#[derive(Debug)]
pub(crate) struct ParsedOffset {
    /// The kind of offset parsed.
    kind: ParsedOffsetKind,
}

impl ParsedOffset {
    /// Convert a parsed offset into a Jiff offset.
    ///
    /// If the offset was parsed from a Zulu designator, then the offset
    /// returned is indistinguishable from `+00` or `-00`.
    ///
    /// # Errors
    ///
    /// A variety of parsing errors are possible.
    ///
    /// Also, beyond normal range checks on the allowed components of a UTC
    /// offset, this does rounding based on the fractional nanosecond part. As
    /// a result, if the parsed value would be rounded to a value not in bounds
    /// for a Jiff offset, this returns an error.
    pub(crate) fn to_offset(&self) -> Result<Offset, Error> {
        match self.kind {
            ParsedOffsetKind::Zulu => Ok(Offset::UTC),
            ParsedOffsetKind::Numeric(ref numeric) => numeric.to_offset(),
        }
    }

    /// Convert a parsed offset to a more structured representation.
    ///
    /// This is like `to_offset`, but preserves `Z` and `-00:00` versus
    /// `+00:00`. This does still attempt to create an `Offset`, and that
    /// construction can fail.
    pub(crate) fn to_pieces_offset(&self) -> Result<PiecesOffset, Error> {
        match self.kind {
            ParsedOffsetKind::Zulu => Ok(PiecesOffset::Zulu),
            ParsedOffsetKind::Numeric(ref numeric) => {
                let mut off = PiecesNumericOffset::from(numeric.to_offset()?);
                if numeric.sign < C(0) {
                    off = off.with_negative_zero();
                }
                Ok(PiecesOffset::from(off))
            }
        }
    }

    /// Whether this parsed offset corresponds to Zulu time or not.
    ///
    /// This is useful in error reporting for parsing civil times. Namely, we
    /// report an error when parsing a civil time with a Zulu offset since it
    /// is almost always the wrong thing to do.
    pub(crate) fn is_zulu(&self) -> bool {
        matches!(self.kind, ParsedOffsetKind::Zulu)
    }

    /// Whether the parsed offset had an explicit sub-minute component or not.
    pub(crate) fn has_subminute(&self) -> bool {
        let ParsedOffsetKind::Numeric(ref numeric) = self.kind else {
            return false;
        };
        numeric.seconds.is_some()
    }
}

/// The kind of a parsed offset.
#[derive(Debug)]
enum ParsedOffsetKind {
    /// The zulu offset, corresponding to UTC in a context where the offset for
    /// civil time is unknown or unavailable.
    Zulu,
    /// The specific numeric offset.
    Numeric(Numeric),
}

/// A numeric representation of a UTC offset.
struct Numeric {
    /// The sign that was parsed from the numeric UTC offset. This is always
    /// either `1` or `-1`, never `0`.
    sign: t::Sign,
    /// The hours component. This is non-optional because every UTC offset must
    /// have at least hours.
    hours: ParsedOffsetHours,
    /// The minutes component.
    minutes: Option<ParsedOffsetMinutes>,
    /// The seconds component. This is only possible when subminute resolution
    /// is enabled.
    seconds: Option<ParsedOffsetSeconds>,
    /// The nanoseconds fractional component. This is only possible when
    /// subminute resolution is enabled.
    nanoseconds: Option<t::SubsecNanosecond>,
}

impl Numeric {
    /// Convert a parsed numeric offset into a Jiff offset.
    ///
    /// This does rounding based on the fractional nanosecond part. As a
    /// result, if the parsed value would be rounded to a value not in bounds
    /// for a Jiff offset, this returns an error.
    fn to_offset(&self) -> Result<Offset, Error> {
        let mut seconds = t::SpanZoneOffset::rfrom(C(3_600) * self.hours);
        if let Some(part_minutes) = self.minutes {
            seconds += C(60) * part_minutes;
        }
        if let Some(part_seconds) = self.seconds {
            seconds += part_seconds;
        }
        if let Some(part_nanoseconds) = self.nanoseconds {
            if part_nanoseconds >= C(500_000_000) {
                seconds = seconds
                    .try_checked_add("offset-seconds", C(1))
                    .with_context(|| {
                        err!(
                            "due to precision loss, UTC offset '{}' is \
                             rounded to a value that is out of bounds",
                            self,
                        )
                    })?;
            }
        }
        Ok(Offset::from_seconds_ranged(seconds * self.sign))
    }
}

// This impl is just used for error messages when converting a `Numeric` to an
// `Offset` fails.
impl core::fmt::Display for Numeric {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.sign == C(-1) {
            write!(f, "-")?;
        } else {
            write!(f, "+")?;
        }
        write!(f, "{:02}", self.hours)?;
        if let Some(minutes) = self.minutes {
            write!(f, ":{:02}", minutes)?;
        }
        if let Some(seconds) = self.seconds {
            write!(f, ":{:02}", seconds)?;
        }
        if let Some(nanos) = self.nanoseconds {
            static FMT: FractionalFormatter = FractionalFormatter::new();
            write!(f, ".{}", FMT.format(i64::from(nanos)).as_str())?;
        }
        Ok(())
    }
}

// We give a succinct Debug impl (identical to Display) to make snapshot
// testing a bit nicer.
impl core::fmt::Debug for Numeric {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

/// A parser for UTC offsets.
///
/// At time of writing, the typical configuration for offset parsing is to
/// enable Zulu support and subminute precision. But when parsing zoned
/// datetimes, and specifically, offsets within time zone annotations (the RFC
/// 9557 extension to RFC 3339), then neither zulu nor subminute support are
/// enabled.
///
/// N.B. I'm not actually totally clear on why zulu/subminute aren't allowed in
/// time zone annotations, but that's what Temporal's grammar seems to dictate.
/// One might argue that this is what RFCs 3339 and 9557 require, but the
/// Temporal grammar is already recognizing a superset anyway.
#[derive(Debug)]
pub(crate) struct Parser {
    zulu: bool,
    require_minute: bool,
    require_second: bool,
    subminute: bool,
    subsecond: bool,
    colon: Colon,
}

impl Parser {
    /// Create a new UTC offset parser with the default configuration.
    pub(crate) const fn new() -> Parser {
        Parser {
            zulu: true,
            require_minute: false,
            require_second: false,
            subminute: true,
            subsecond: true,
            colon: Colon::Optional,
        }
    }

    /// When enabled, the `z` and `Z` designators are recognized as a "zulu"
    /// indicator for UTC when the civil time offset is unknown or unavailable.
    ///
    /// When disabled, neither `z` nor `Z` will be recognized and a parser
    /// error will occur if one is found.
    ///
    /// This is enabled by default.
    pub(crate) const fn zulu(self, yes: bool) -> Parser {
        Parser { zulu: yes, ..self }
    }

    /// When enabled, the minute component of a time zone offset is required.
    /// If no minutes are found, then an error is returned.
    ///
    /// This is disabled by default.
    pub(crate) const fn require_minute(self, yes: bool) -> Parser {
        Parser { require_minute: yes, ..self }
    }

    /// When enabled, the second component of a time zone offset is required.
    /// If no seconds (or minutes) are found, then an error is returned.
    ///
    /// When `subminute` is disabled, this setting has no effect.
    ///
    /// This is disabled by default.
    pub(crate) const fn require_second(self, yes: bool) -> Parser {
        Parser { require_second: yes, ..self }
    }

    /// When enabled, offsets with precision greater than integral minutes
    /// are supported. Specifically, when enabled, nanosecond precision is
    /// supported.
    ///
    /// When disabled, offsets must be integral minutes. And the `subsecond`
    /// option is ignored.
    pub(crate) const fn subminute(self, yes: bool) -> Parser {
        Parser { subminute: yes, ..self }
    }

    /// When enabled, offsets with precision greater than integral seconds
    /// are supported. Specifically, when enabled, nanosecond precision is
    /// supported. Note though that when a fractional second is found, it is
    /// used to round to the nearest second. (Jiff's `Offset` type only has
    /// second resolution.)
    ///
    /// When disabled, offsets must be integral seconds (or integrate minutes
    /// if the `subminute` option is disabled as well).
    ///
    /// This is ignored if `subminute` is disabled.
    pub(crate) const fn subsecond(self, yes: bool) -> Parser {
        Parser { subsecond: yes, ..self }
    }

    /// Sets how to handle parsing of colons in a time zone offset.
    ///
    /// This is set to `Colon::Optional` by default.
    pub(crate) const fn colon(self, colon: Colon) -> Parser {
        Parser { colon, ..self }
    }

    /// Parse an offset from the beginning of `input`.
    ///
    /// If no offset could be found or it was otherwise invalid, then an error
    /// is returned.
    ///
    /// In general, parsing stops when, after all required components are seen,
    /// an optional component is not present (either because of the end of the
    /// input or because of a character that cannot possibly begin said optional
    /// component). This does mean that there are some corner cases where error
    /// messages will not be as good as they possibly can be. But there are
    /// two exceptions here:
    ///
    /// 1. When Zulu support is disabled and a `Z` or `z` are found, then an
    /// error is returned indicating that `Z` was recognized but specifically
    /// not allowed.
    /// 2. When subminute precision is disabled and a `:` is found after the
    /// minutes component, then an error is returned indicating that the
    /// seconds component was recognized but specifically not allowed.
    ///
    /// Otherwise, for example, if `input` is `-0512:34`, then the `-0512`
    /// will be parsed as `-5 hours, 12 minutes` with an offset of `5`.
    /// Presumably, whatever higher level parser is invoking this routine will
    /// then see an unexpected `:`. But it's likely that a better error message
    /// would call out the fact that mixed basic and extended formats (from
    /// ISO 8601) aren't allowed, and that the offset needs to be written as
    /// either `-05:12:34` or `-051234`. But... these are odd corner cases, so
    /// we abide them.
    pub(crate) fn parse<'i>(
        &self,
        mut input: &'i [u8],
    ) -> Result<Parsed<'i, ParsedOffset>, Error> {
        if input.is_empty() {
            return Err(err!("expected UTC offset, but found end of input"));
        }

        if input[0] == b'Z' || input[0] == b'z' {
            if !self.zulu {
                return Err(err!(
                    "found {z:?} in {original:?} where a numeric UTC offset \
                     was expected (this context does not permit \
                     the Zulu offset)",
                    z = escape::Byte(input[0]),
                    original = escape::Bytes(input),
                ));
            }
            input = &input[1..];
            let value = ParsedOffset { kind: ParsedOffsetKind::Zulu };
            return Ok(Parsed { value, input });
        }
        let Parsed { value: numeric, input } = self.parse_numeric(input)?;
        let value = ParsedOffset { kind: ParsedOffsetKind::Numeric(numeric) };
        Ok(Parsed { value, input })
    }

    /// Like `parse`, but will return `None` if `input` cannot possibly start
    /// with an offset.
    ///
    /// Basically, if `input` is empty, or is not one of `z`, `Z`, `+` or `-`
    /// then this returns `None`.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn parse_optional<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, Option<ParsedOffset>>, Error> {
        let Some(first) = input.first().copied() else {
            return Ok(Parsed { value: None, input });
        };
        if !matches!(first, b'z' | b'Z' | b'+' | b'-') {
            return Ok(Parsed { value: None, input });
        }
        let Parsed { value, input } = self.parse(input)?;
        Ok(Parsed { value: Some(value), input })
    }

    /// Parses a numeric offset from the beginning of `input`.
    ///
    /// The beginning of the input is expected to start with a `+` or a `-`.
    /// Any other case (including an empty string) will result in an error.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_numeric<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, Numeric>, Error> {
        let original = escape::Bytes(input);

        // Parse sign component.
        let Parsed { value: sign, input } =
            self.parse_sign(input).with_context(|| {
                err!("failed to parse sign in UTC numeric offset {original:?}")
            })?;

        // Parse hours component.
        let Parsed { value: hours, input } =
            self.parse_hours(input).with_context(|| {
                err!(
                    "failed to parse hours in UTC numeric offset {original:?}"
                )
            })?;
        let extended = match self.colon {
            Colon::Optional => input.starts_with(b":"),
            Colon::Required => {
                if !input.is_empty() && !input.starts_with(b":") {
                    return Err(err!(
                        "parsed hour component of time zone offset from \
                         {original:?}, but could not find required colon \
                         separator",
                    ));
                }
                true
            }
            Colon::Absent => {
                if !input.is_empty() && input.starts_with(b":") {
                    return Err(err!(
                        "parsed hour component of time zone offset from \
                         {original:?}, but found colon after hours which \
                         is not allowed",
                    ));
                }
                false
            }
        };

        // Start building up our numeric offset value.
        let mut numeric = Numeric {
            sign,
            hours,
            minutes: None,
            seconds: None,
            nanoseconds: None,
        };

        // Parse optional separator after hours.
        let Parsed { value: has_minutes, input } =
            self.parse_separator(input, extended).with_context(|| {
                err!(
                    "failed to parse separator after hours in \
                     UTC numeric offset {original:?}"
                )
            })?;
        if !has_minutes {
            if self.require_minute || (self.subminute && self.require_second) {
                return Err(err!(
                    "parsed hour component of time zone offset from \
                     {original:?}, but could not find required minute \
                     component",
                ));
            }
            return Ok(Parsed { value: numeric, input });
        }

        // Parse minutes component.
        let Parsed { value: minutes, input } =
            self.parse_minutes(input).with_context(|| {
                err!(
                    "failed to parse minutes in UTC numeric offset \
                     {original:?}"
                )
            })?;
        numeric.minutes = Some(minutes);

        // If subminute resolution is not supported, then we're done here.
        if !self.subminute {
            // While we generally try to "stop" parsing once we're done
            // seeing things we expect, in this case, if we see a colon, it
            // almost certainly indicates that someone has tried to provide
            // more precision than is supported. So we return an error here.
            // If this winds up being problematic, we can make this error
            // configurable or remove it altogether (unfortunate).
            if input.get(0).map_or(false, |&b| b == b':') {
                return Err(err!(
                    "subminute precision for UTC numeric offset {original:?} \
                     is not enabled in this context (must provide only \
                     integral minutes)",
                ));
            }
            return Ok(Parsed { value: numeric, input });
        }

        // Parse optional separator after minutes.
        let Parsed { value: has_seconds, input } =
            self.parse_separator(input, extended).with_context(|| {
                err!(
                    "failed to parse separator after minutes in \
                     UTC numeric offset {original:?}"
                )
            })?;
        if !has_seconds {
            if self.require_second {
                return Err(err!(
                    "parsed hour and minute components of time zone offset \
                     from {original:?}, but could not find required second \
                     component",
                ));
            }
            return Ok(Parsed { value: numeric, input });
        }

        // Parse seconds component.
        let Parsed { value: seconds, input } =
            self.parse_seconds(input).with_context(|| {
                err!(
                    "failed to parse seconds in UTC numeric offset \
                     {original:?}"
                )
            })?;
        numeric.seconds = Some(seconds);

        // If subsecond resolution is not supported, then we're done here.
        if !self.subsecond {
            if input.get(0).map_or(false, |&b| b == b'.' || b == b',') {
                return Err(err!(
                    "subsecond precision for UTC numeric offset {original:?} \
                     is not enabled in this context (must provide only \
                     integral minutes or seconds)",
                ));
            }
            return Ok(Parsed { value: numeric, input });
        }

        // Parse an optional fractional component.
        let Parsed { value: nanoseconds, input } =
            parse_temporal_fraction(input).with_context(|| {
                err!(
                    "failed to parse fractional nanoseconds in \
                     UTC numeric offset {original:?}",
                )
            })?;
        numeric.nanoseconds = nanoseconds;
        Ok(Parsed { value: numeric, input })
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_sign<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, t::Sign>, Error> {
        let sign = input.get(0).copied().ok_or_else(|| {
            err!("expected UTC numeric offset, but found end of input")
        })?;
        let sign = if sign == b'+' {
            t::Sign::N::<1>()
        } else if sign == b'-' {
            t::Sign::N::<-1>()
        } else {
            return Err(err!(
                "expected '+' or '-' sign at start of UTC numeric offset, \
                 but found {found:?} instead",
                found = escape::Byte(sign),
            ));
        };
        Ok(Parsed { value: sign, input: &input[1..] })
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_hours<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, ParsedOffsetHours>, Error> {
        let (hours, input) = parse::split(input, 2).ok_or_else(|| {
            err!("expected two digit hour after sign, but found end of input",)
        })?;
        let hours = parse::i64(hours).with_context(|| {
            err!(
                "failed to parse {hours:?} as hours (a two digit integer)",
                hours = escape::Bytes(hours),
            )
        })?;
        // Note that we support a slightly bigger range of offsets than
        // Temporal. Temporal seems to support only up to 23 hours, but
        // we go up to 25 hours. This is done to support POSIX time zone
        // strings, which also require 25 hours (plus the maximal minute/second
        // components).
        let hours = ParsedOffsetHours::try_new("hours", hours)
            .context("offset hours are not valid")?;
        Ok(Parsed { value: hours, input })
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_minutes<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, ParsedOffsetMinutes>, Error> {
        let (minutes, input) = parse::split(input, 2).ok_or_else(|| {
            err!(
                "expected two digit minute after hours, \
                 but found end of input",
            )
        })?;
        let minutes = parse::i64(minutes).with_context(|| {
            err!(
                "failed to parse {minutes:?} as minutes (a two digit integer)",
                minutes = escape::Bytes(minutes),
            )
        })?;
        let minutes = ParsedOffsetMinutes::try_new("minutes", minutes)
            .context("minutes are not valid")?;
        Ok(Parsed { value: minutes, input })
    }

    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_seconds<'i>(
        &self,
        input: &'i [u8],
    ) -> Result<Parsed<'i, ParsedOffsetSeconds>, Error> {
        let (seconds, input) = parse::split(input, 2).ok_or_else(|| {
            err!(
                "expected two digit second after hours, \
                 but found end of input",
            )
        })?;
        let seconds = parse::i64(seconds).with_context(|| {
            err!(
                "failed to parse {seconds:?} as seconds (a two digit integer)",
                seconds = escape::Bytes(seconds),
            )
        })?;
        let seconds = ParsedOffsetSeconds::try_new("seconds", seconds)
            .context("time zone offset seconds are not valid")?;
        Ok(Parsed { value: seconds, input })
    }

    /// Parses a separator between hours/minutes or minutes/seconds. When
    /// `true` is returned, we expect to parse the next component. When `false`
    /// is returned, then no separator was found and there is no expectation of
    /// finding another component.
    ///
    /// When in extended mode, true is returned if and only if a separator is
    /// found.
    ///
    /// When in basic mode (not extended), then a subsequent component is only
    /// expected when `input` begins with two ASCII digits.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    fn parse_separator<'i>(
        &self,
        mut input: &'i [u8],
        extended: bool,
    ) -> Result<Parsed<'i, bool>, Error> {
        if !extended {
            let expected =
                input.len() >= 2 && input[..2].iter().all(u8::is_ascii_digit);
            return Ok(Parsed { value: expected, input });
        }
        let is_separator = input.get(0).map_or(false, |&b| b == b':');
        if is_separator {
            input = &input[1..];
        }
        Ok(Parsed { value: is_separator, input })
    }
}

/// How to handle parsing of colons in a time zone offset.
#[derive(Debug)]
pub(crate) enum Colon {
    /// Colons may be present or not. When present, colons must be used
    /// consistently. For example, `+05:3015` and `-0530:15` are not allowed.
    Optional,
    /// Colons must be present.
    Required,
    /// Colons must be absent.
    Absent,
}

#[cfg(test)]
mod tests {
    use crate::util::rangeint::RInto;

    use super::*;

    #[test]
    fn ok_zulu() {
        let p = |input| Parser::new().parse(input).unwrap();

        insta::assert_debug_snapshot!(p(b"Z"), @r###"
        Parsed {
            value: ParsedOffset {
                kind: Zulu,
            },
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"z"), @r###"
        Parsed {
            value: ParsedOffset {
                kind: Zulu,
            },
            input: "",
        }
        "###);
    }

    #[test]
    fn ok_numeric() {
        let p = |input| Parser::new().parse(input).unwrap();

        insta::assert_debug_snapshot!(p(b"-05"), @r###"
        Parsed {
            value: ParsedOffset {
                kind: Numeric(
                    -05,
                ),
            },
            input: "",
        }
        "###);
    }

    // Successful parse tests where the offset ends at the end of the string.
    #[test]
    fn ok_numeric_complete() {
        let p = |input| Parser::new().parse_numeric(input).unwrap();

        insta::assert_debug_snapshot!(p(b"-05"), @r###"
        Parsed {
            value: -05,
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"+05"), @r###"
        Parsed {
            value: +05,
            input: "",
        }
        "###);

        insta::assert_debug_snapshot!(p(b"+25:59"), @r###"
        Parsed {
            value: +25:59,
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"+2559"), @r###"
        Parsed {
            value: +25:59,
            input: "",
        }
        "###);

        insta::assert_debug_snapshot!(p(b"+25:59:59"), @r###"
        Parsed {
            value: +25:59:59,
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"+255959"), @r###"
        Parsed {
            value: +25:59:59,
            input: "",
        }
        "###);

        insta::assert_debug_snapshot!(p(b"+25:59:59.999"), @r###"
        Parsed {
            value: +25:59:59.999,
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"+25:59:59,999"), @r###"
        Parsed {
            value: +25:59:59.999,
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"+255959.999"), @r###"
        Parsed {
            value: +25:59:59.999,
            input: "",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"+255959,999"), @r###"
        Parsed {
            value: +25:59:59.999,
            input: "",
        }
        "###);

        insta::assert_debug_snapshot!(p(b"+25:59:59.999999999"), @r###"
        Parsed {
            value: +25:59:59.999999999,
            input: "",
        }
        "###);
    }

    // Successful parse tests where the offset ends before the end of the
    // string.
    #[test]
    fn ok_numeric_incomplete() {
        let p = |input| Parser::new().parse_numeric(input).unwrap();

        insta::assert_debug_snapshot!(p(b"-05a"), @r###"
        Parsed {
            value: -05,
            input: "a",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"-05:12a"), @r###"
        Parsed {
            value: -05:12,
            input: "a",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"-05:12."), @r###"
        Parsed {
            value: -05:12,
            input: ".",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"-05:12,"), @r###"
        Parsed {
            value: -05:12,
            input: ",",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"-0512a"), @r###"
        Parsed {
            value: -05:12,
            input: "a",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"-0512:"), @r###"
        Parsed {
            value: -05:12,
            input: ":",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"-05:12:34a"), @r###"
        Parsed {
            value: -05:12:34,
            input: "a",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"-05:12:34.9a"), @r###"
        Parsed {
            value: -05:12:34.9,
            input: "a",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"-05:12:34.9."), @r###"
        Parsed {
            value: -05:12:34.9,
            input: ".",
        }
        "###);
        insta::assert_debug_snapshot!(p(b"-05:12:34.9,"), @r###"
        Parsed {
            value: -05:12:34.9,
            input: ",",
        }
        "###);
    }

    // An empty string is invalid. The parser is written from the perspective
    // that if it's called, then the caller expects a numeric UTC offset at
    // that position.
    #[test]
    fn err_numeric_empty() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"").unwrap_err(),
            @r###"failed to parse sign in UTC numeric offset "": expected UTC numeric offset, but found end of input"###,
        );
    }

    // A numeric offset always has to begin with a '+' or a '-'.
    #[test]
    fn err_numeric_notsign() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"*").unwrap_err(),
            @r###"failed to parse sign in UTC numeric offset "*": expected '+' or '-' sign at start of UTC numeric offset, but found "*" instead"###,
        );
    }

    // The hours component must be at least two bytes.
    #[test]
    fn err_numeric_hours_too_short() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"+a").unwrap_err(),
            @r###"failed to parse hours in UTC numeric offset "+a": expected two digit hour after sign, but found end of input"###,
        );
    }

    // The hours component must be at least two ASCII digits.
    #[test]
    fn err_numeric_hours_invalid_digits() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"+ab").unwrap_err(),
            @r###"failed to parse hours in UTC numeric offset "+ab": failed to parse "ab" as hours (a two digit integer): invalid digit, expected 0-9 but got a"###,
        );
    }

    // The hours component must be in range.
    #[test]
    fn err_numeric_hours_out_of_range() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"-26").unwrap_err(),
            @r###"failed to parse hours in UTC numeric offset "-26": offset hours are not valid: parameter 'hours' with value 26 is not in the required range of 0..=25"###,
        );
    }

    // The minutes component must be at least two bytes.
    #[test]
    fn err_numeric_minutes_too_short() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"+05:a").unwrap_err(),
            @r###"failed to parse minutes in UTC numeric offset "+05:a": expected two digit minute after hours, but found end of input"###,
        );
    }

    // The minutes component must be at least two ASCII digits.
    #[test]
    fn err_numeric_minutes_invalid_digits() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"+05:ab").unwrap_err(),
            @r###"failed to parse minutes in UTC numeric offset "+05:ab": failed to parse "ab" as minutes (a two digit integer): invalid digit, expected 0-9 but got a"###,
        );
    }

    // The minutes component must be in range.
    #[test]
    fn err_numeric_minutes_out_of_range() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"-05:60").unwrap_err(),
            @r###"failed to parse minutes in UTC numeric offset "-05:60": minutes are not valid: parameter 'minutes' with value 60 is not in the required range of 0..=59"###,
        );
    }

    // The seconds component must be at least two bytes.
    #[test]
    fn err_numeric_seconds_too_short() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"+05:30:a").unwrap_err(),
            @r###"failed to parse seconds in UTC numeric offset "+05:30:a": expected two digit second after hours, but found end of input"###,
        );
    }

    // The seconds component must be at least two ASCII digits.
    #[test]
    fn err_numeric_seconds_invalid_digits() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"+05:30:ab").unwrap_err(),
            @r###"failed to parse seconds in UTC numeric offset "+05:30:ab": failed to parse "ab" as seconds (a two digit integer): invalid digit, expected 0-9 but got a"###,
        );
    }

    // The seconds component must be in range.
    #[test]
    fn err_numeric_seconds_out_of_range() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"-05:30:60").unwrap_err(),
            @r###"failed to parse seconds in UTC numeric offset "-05:30:60": time zone offset seconds are not valid: parameter 'seconds' with value 60 is not in the required range of 0..=59"###,
        );
    }

    // The fraction component, if present as indicated by a separator, must be
    // non-empty.
    #[test]
    fn err_numeric_fraction_non_empty() {
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"-05:30:44.").unwrap_err(),
            @r###"failed to parse fractional nanoseconds in UTC numeric offset "-05:30:44.": found decimal after seconds component, but did not find any decimal digits after decimal"###,
        );
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"-05:30:44,").unwrap_err(),
            @r###"failed to parse fractional nanoseconds in UTC numeric offset "-05:30:44,": found decimal after seconds component, but did not find any decimal digits after decimal"###,
        );

        // Instead of end-of-string, add invalid digit.
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"-05:30:44.a").unwrap_err(),
            @r###"failed to parse fractional nanoseconds in UTC numeric offset "-05:30:44.a": found decimal after seconds component, but did not find any decimal digits after decimal"###,
        );
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"-05:30:44,a").unwrap_err(),
            @r###"failed to parse fractional nanoseconds in UTC numeric offset "-05:30:44,a": found decimal after seconds component, but did not find any decimal digits after decimal"###,
        );

        // And also test basic format.
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"-053044.a").unwrap_err(),
            @r###"failed to parse fractional nanoseconds in UTC numeric offset "-053044.a": found decimal after seconds component, but did not find any decimal digits after decimal"###,
        );
        insta::assert_snapshot!(
            Parser::new().parse_numeric(b"-053044,a").unwrap_err(),
            @r###"failed to parse fractional nanoseconds in UTC numeric offset "-053044,a": found decimal after seconds component, but did not find any decimal digits after decimal"###,
        );
    }

    // A special case where it is clear that sub-minute precision has been
    // requested, but that it is has been forcefully disabled. This error is
    // meant to make what is likely a subtle failure mode more explicit.
    #[test]
    fn err_numeric_subminute_disabled_but_desired() {
        insta::assert_snapshot!(
            Parser::new().subminute(false).parse_numeric(b"-05:59:32").unwrap_err(),
            @r###"subminute precision for UTC numeric offset "-05:59:32" is not enabled in this context (must provide only integral minutes)"###,
        );
    }

    // Another special case where Zulu parsing has been explicitly disabled,
    // but a Zulu string was found.
    #[test]
    fn err_zulu_disabled_but_desired() {
        insta::assert_snapshot!(
            Parser::new().zulu(false).parse(b"Z").unwrap_err(),
            @r###"found "Z" in "Z" where a numeric UTC offset was expected (this context does not permit the Zulu offset)"###,
        );
        insta::assert_snapshot!(
            Parser::new().zulu(false).parse(b"z").unwrap_err(),
            @r###"found "z" in "z" where a numeric UTC offset was expected (this context does not permit the Zulu offset)"###,
        );
    }

    // Once a `Numeric` has been parsed, it is almost possible to assume that
    // it can be infallibly converted to an `Offset`. The one case where this
    // isn't true is when there is a fractional nanosecond part along with
    // maximal
    #[test]
    fn err_numeric_too_big_for_offset() {
        let numeric = Numeric {
            sign: t::Sign::MAX_SELF,
            hours: ParsedOffsetHours::MAX_SELF,
            minutes: Some(ParsedOffsetMinutes::MAX_SELF),
            seconds: Some(ParsedOffsetSeconds::MAX_SELF),
            nanoseconds: Some(C(499_999_999).rinto()),
        };
        assert_eq!(numeric.to_offset().unwrap(), Offset::MAX);

        let numeric = Numeric {
            sign: t::Sign::MAX_SELF,
            hours: ParsedOffsetHours::MAX_SELF,
            minutes: Some(ParsedOffsetMinutes::MAX_SELF),
            seconds: Some(ParsedOffsetSeconds::MAX_SELF),
            nanoseconds: Some(C(500_000_000).rinto()),
        };
        insta::assert_snapshot!(
            numeric.to_offset().unwrap_err(),
            @"due to precision loss, UTC offset '+25:59:59.5' is rounded to a value that is out of bounds: parameter 'offset-seconds' with value 1 is not in the required range of -93599..=93599",
        );
    }

    // Same as numeric_too_big_for_offset, but at the minimum boundary.
    #[test]
    fn err_numeric_too_small_for_offset() {
        let numeric = Numeric {
            sign: t::Sign::MIN_SELF,
            hours: ParsedOffsetHours::MAX_SELF,
            minutes: Some(ParsedOffsetMinutes::MAX_SELF),
            seconds: Some(ParsedOffsetSeconds::MAX_SELF),
            nanoseconds: Some(C(499_999_999).rinto()),
        };
        assert_eq!(numeric.to_offset().unwrap(), Offset::MIN);

        let numeric = Numeric {
            sign: t::Sign::MIN_SELF,
            hours: ParsedOffsetHours::MAX_SELF,
            minutes: Some(ParsedOffsetMinutes::MAX_SELF),
            seconds: Some(ParsedOffsetSeconds::MAX_SELF),
            nanoseconds: Some(C(500_000_000).rinto()),
        };
        insta::assert_snapshot!(
            numeric.to_offset().unwrap_err(),
            @"due to precision loss, UTC offset '-25:59:59.5' is rounded to a value that is out of bounds: parameter 'offset-seconds' with value 1 is not in the required range of -93599..=93599",
        );
    }
}
