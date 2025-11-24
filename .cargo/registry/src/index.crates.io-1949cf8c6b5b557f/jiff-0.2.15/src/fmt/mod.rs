/*!
Configurable support for printing and parsing datetimes and durations.

Note that for most use cases, you should be using the corresponding
[`Display`](std::fmt::Display) or [`FromStr`](std::str::FromStr) trait
implementations for printing and parsing respectively. The APIs in this module
provide more configurable support for printing and parsing.

# Tables of examples

The tables below attempt to show some examples of datetime and duration
formatting, along with names and links to relevant routines and types. The
point of these tables is to give a general overview of the formatting and
parsing functionality in these sub-modules.

## Support for `FromStr` and `Display`

This table lists the formats supported by the [`FromStr`] and [`Display`]
trait implementations on the datetime and duration types in Jiff.

In all of these cases, the trait implementations are mere conveniences for
functionality provided by the [`temporal`] sub-module (and, in a couple cases,
the [`friendly`] sub-module). The sub-modules provide lower level control
(such as parsing from `&[u8]`) and more configuration (such as controlling the
disambiguation strategy used when parsing zoned datetime [RFC-9557] strings).

| Example | Format | Links |
| ------- | ------ | ----- |
| `2025-08-20T17:35:00Z` | [RFC-3339] | [`Timestamp`] |
| `2025-08-20T17:35:00-05` | [RFC-3339] | [`FromStr`] impl and<br>[`Timestamp::display_with_offset`] |
| `2025-08-20T17:35:00+02[Poland]` | [RFC-9557] | [`Zoned`] |
| `2025-08-20T17:35:00+02:00[+02:00]` | [RFC-9557] | [`Zoned`] |
| `2025-08-20T17:35:00` | [ISO-8601] | [`civil::DateTime`] |
| `2025-08-20` | [ISO-8601] | [`civil::Date`] |
| `17:35:00` | [ISO-8601] | [`civil::Time`] |
| `P1Y2M3W4DT5H6M7S` | [ISO-8601], [Temporal] | [`Span`] |
| `PT1H2M3S` | [ISO-8601] | [`SignedDuration`], [`Span`] |
| `PT1H2M3.123456789S` | [ISO-8601] | [`SignedDuration`], [`Span`] |
| `1d 2h 3m 5s` | [`friendly`] | [`FromStr`] impl and alternative [`Display`]<br>via `{:#}` for [`SignedDuration`], [`Span`] |

Note that for datetimes like `2025-08-20T17:35:00`, the following variants are
also accepted:

```text
2025-08-20 17:35:00
2025-08-20T17:35:00.123456789
2025-08-20T17:35
2025-08-20T17
```

This applies to RFC 3339 and RFC 9557 timestamps as well.

Also, for ISO 8601 durations, the unit designator labels are matched
case insensitively. For example, `PT1h2m3s` is recognized by Jiff.

## The "friendly" duration format

This table lists a few examples of the [`friendly`] duration format. Briefly,
it is a bespoke format for Jiff, but is meant to match similar bespoke formats
used elsewhere and be easier to read than the standard ISO 8601 duration
format.

All examples below can be parsed via a [`Span`]'s [`FromStr`] trait
implementation. All examples with units no bigger than hours can be parsed via
a [`SignedDuration`]'s [`FromStr`] trait implementation. This table otherwise
shows the options for printing durations in the format shown.

| Example | Print configuration |
| ------- | ------------------- |
| `1year 2months` | [`Designator::Verbose`] via [`SpanPrinter::designator`] |
| `1yr 2mos` | [`Designator::Short`] via [`SpanPrinter::designator`] |
| `1y 2mo` | [`Designator::Compact`] via [`SpanPrinter::designator`] (default) |
| `1h2m3s` | [`Spacing::None`] via [`SpanPrinter::spacing`] |
| `1h 2m 3s` | [`Spacing::BetweenUnits`] via [`SpanPrinter::spacing`] (default) |
| `1 h 2 m 3 s` | [`Spacing::BetweenUnitsAndDesignators`] via [`SpanPrinter::spacing`] |
| `2d 3h ago` | [`Direction::Auto`] via [`SpanPrinter::direction`] (default) |
| `-2d 3h` | [`Direction::Sign`] via [`SpanPrinter::direction`] |
| `+2d 3h` | [`Direction::ForceSign`] via [`SpanPrinter::direction`] |
| `2d 3h ago` | [`Direction::Suffix`] via [`SpanPrinter::direction`] |
| `9.123456789s` | [`FractionalUnit::Second`] via [`SpanPrinter::fractional`] |
| `1y, 2mo` | [`SpanPrinter::comma_after_designator`] |
| `15d 02:59:15.123` | [`SpanPrinter::hours_minutes_seconds`] |

## Bespoke datetime formats via `strptime` and `strftime`

Every datetime type has bespoke formatting routines defined on it. For
example, [`Zoned::strptime`] and [`civil::Date::strftime`]. Additionally, the
[`strtime`] sub-module also provides convenience routines, [`strtime::format`]
and [`strtime::parse`], where the former is generic over any datetime type in
Jiff and the latter provides a [`BrokenDownTime`] for granular parsing.

| Example | Format string |
| ------- | ------------- |
| `2025-05-20` | `%Y-%m-%d` |
| `2025-05-20` | `%F` |
| `2025-W21-2` | `%G-W%V-%u` |
| `05/20/25` | `%m/%d/%y` |
| `Monday, February 10, 2025 at 9:01pm -0500` | `%A, %B %d, %Y at %-I:%M%P %z` |
| `Monday, February 10, 2025 at 9:01pm EST` | `%A, %B %d, %Y at %-I:%M%P %Z` |
| `Monday, February 10, 2025 at 9:01pm America/New_York` | `%A, %B %d, %Y at %-I:%M%P %Q` |

The specific conversion specifiers supported are documented in the [`strtime`]
sub-module. While precise POSIX compatibility is not guaranteed, the conversion
specifiers are generally meant to match prevailing implementations. (Although
there are many such implementations and they each tend to have their own quirks
and features.)

## RFC 2822 parsing and printing

[RFC-2822] support is provided by the [`rfc2822`] sub-module.

| Example | Links |
| ------- | ----- |
| `Thu, 29 Feb 2024 05:34 -0500` | [`rfc2822::parse`] and [`rfc2822::to_string`] |
| `Thu, 01 Jan 1970 00:00:01 GMT` | [`DateTimePrinter::timestamp_to_rfc9110_string`] |

[Temporal]: https://tc39.es/proposal-temporal/#sec-temporal-iso8601grammar
[ISO-8601]: https://www.iso.org/iso-8601-date-and-time-format.html
[RFC-3339]: https://www.rfc-editor.org/rfc/rfc3339
[RFC-9557]: https://www.rfc-editor.org/rfc/rfc9557.html
[ISO-8601]: https://www.iso.org/iso-8601-date-and-time-format.html
[RFC-2822]: https://datatracker.ietf.org/doc/html/rfc2822
[RFC-9110]: https://datatracker.ietf.org/doc/html/rfc9110#section-5.6.7-15
[`Display`]: std::fmt::Display
[`FromStr`]: std::str::FromStr
[`friendly`]: crate::fmt::friendly
[`temporal`]: crate::fmt::temporal
[`rfc2822`]: crate::fmt::rfc2822
[`strtime`]: crate::fmt::strtime
[`civil::DateTime`]: crate::civil::DateTime
[`civil::Date`]: crate::civil::Date
[`civil::Date::strftime`]: crate::civil::Date::strftime
[`civil::Time`]: crate::civil::Time
[`SignedDuration`]: crate::SignedDuration
[`Span`]: crate::Span
[`Timestamp`]: crate::Timestamp
[`Timestamp::display_with_offset`]: crate::Timestamp::display_with_offset
[`Zoned`]: crate::Zoned
[`Zoned::strptime`]: crate::Zoned::strptime

[`Designator::Verbose`]: crate::fmt::friendly::Designator::Verbose
[`Designator::Short`]: crate::fmt::friendly::Designator::Short
[`Designator::Compact`]: crate::fmt::friendly::Designator::Compact
[`Spacing::None`]: crate::fmt::friendly::Spacing::None
[`Spacing::BetweenUnits`]: crate::fmt::friendly::Spacing::BetweenUnits
[`Spacing::BetweenUnitsAndDesignators`]: crate::fmt::friendly::Spacing::BetweenUnitsAndDesignators
[`Direction::Auto`]: crate::fmt::friendly::Direction::Auto
[`Direction::Sign`]: crate::fmt::friendly::Direction::Sign
[`Direction::ForceSign`]: crate::fmt::friendly::Direction::ForceSign
[`Direction::Suffix`]: crate::fmt::friendly::Direction::Suffix
[`FractionalUnit::Second`]: crate::fmt::friendly::FractionalUnit::Second
[`SpanPrinter::designator`]: crate::fmt::friendly::SpanPrinter::designator
[`SpanPrinter::spacing`]: crate::fmt::friendly::SpanPrinter::spacing
[`SpanPrinter::direction`]: crate::fmt::friendly::SpanPrinter::direction
[`SpanPrinter::fractional`]: crate::fmt::friendly::SpanPrinter::fractional
[`SpanPrinter::comma_after_designator`]: crate::fmt::friendly::SpanPrinter::comma_after_designator
[`SpanPrinter::hours_minutes_seconds`]: crate::fmt::friendly::SpanPrinter::hours_minutes_seconds

[`BrokenDownTime`]: crate::fmt::strtime::BrokenDownTime
[`strtime::parse`]: crate::fmt::strtime::parse
[`strtime::format`]: crate::fmt::strtime::format

[`rfc2822::parse`]: crate::fmt::rfc2822::parse
[`rfc2822::to_string`]: crate::fmt::rfc2822::to_string
[`DateTimePrinter::timestamp_to_rfc9110_string`]: crate::fmt::rfc2822::DateTimePrinter::timestamp_to_rfc9110_string
*/

use crate::{
    error::{err, Error},
    util::escape,
};

use self::util::{Decimal, DecimalFormatter, Fractional, FractionalFormatter};

pub mod friendly;
mod offset;
pub mod rfc2822;
mod rfc9557;
#[cfg(feature = "serde")]
pub mod serde;
pub mod strtime;
pub mod temporal;
mod util;

/// The result of parsing a value out of a slice of bytes.
///
/// This contains both the parsed value and the offset at which the value
/// ended in the input given. This makes it possible to parse, for example, a
/// datetime value as a prefix of some larger string without knowing ahead of
/// time where it ends.
#[derive(Clone)]
pub(crate) struct Parsed<'i, V> {
    /// The value parsed.
    value: V,
    /// The remaining unparsed input.
    input: &'i [u8],
}

impl<'i, V: core::fmt::Display> Parsed<'i, V> {
    /// Ensures that the parsed value represents the entire input. This occurs
    /// precisely when the `input` on this parsed value is empty.
    ///
    /// This is useful when one expects a parsed value to consume the entire
    /// input, and to consider it an error if it doesn't.
    #[inline]
    fn into_full(self) -> Result<V, Error> {
        if self.input.is_empty() {
            return Ok(self.value);
        }
        Err(err!(
            "parsed value '{value}', but unparsed input {unparsed:?} \
             remains (expected no unparsed input)",
            value = self.value,
            unparsed = escape::Bytes(self.input),
        ))
    }
}

impl<'i, V: core::fmt::Debug> core::fmt::Debug for Parsed<'i, V> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Parsed")
            .field("value", &self.value)
            .field("input", &escape::Bytes(self.input))
            .finish()
    }
}

/// A trait for printing datetimes or spans into Unicode-accepting buffers or
/// streams.
///
/// The most useful implementations of this trait are for the `String` and
/// `Vec<u8>` types. But any implementation of [`std::fmt::Write`] and
/// [`std::io::Write`] can be used via the [`StdFmtWrite`] and [`StdIoWrite`]
/// adapters, respectively.
///
/// Most users of Jiff should not need to interact with this trait directly.
/// Instead, printing is handled via the [`Display`](std::fmt::Display)
/// implementation of the relevant type.
///
/// # Design
///
/// This trait is a near-clone of the `std::fmt::Write` trait. It's also very
/// similar to the `std::io::Write` trait, but like `std::fmt::Write`, this
/// trait is limited to writing valid UTF-8. The UTF-8 restriction was adopted
/// because we really want to support printing datetimes and spans to `String`
/// buffers. If we permitted writing `&[u8]` data, then writing to a `String`
/// buffer would always require a costly UTF-8 validation check.
///
/// The `std::fmt::Write` trait wasn't used itself because:
///
/// 1. Using a custom trait allows us to require using Jiff's error type.
/// (Although this extra flexibility isn't currently used, since printing only
/// fails when writing to the underlying buffer or stream fails.)
/// 2. Using a custom trait allows us more control over the implementations of
/// the trait. For example, a custom trait means we can format directly into
/// a `Vec<u8>` buffer, which isn't possible with `std::fmt::Write` because
/// there is no `std::fmt::Write` trait implementation for `Vec<u8>`.
pub trait Write {
    /// Write the given string to this writer, returning whether the write
    /// succeeded or not.
    fn write_str(&mut self, string: &str) -> Result<(), Error>;

    /// Write the given character to this writer, returning whether the write
    /// succeeded or not.
    #[inline]
    fn write_char(&mut self, char: char) -> Result<(), Error> {
        self.write_str(char.encode_utf8(&mut [0; 4]))
    }
}

#[cfg(any(test, feature = "alloc"))]
impl Write for alloc::string::String {
    #[inline]
    fn write_str(&mut self, string: &str) -> Result<(), Error> {
        self.push_str(string);
        Ok(())
    }
}

#[cfg(any(test, feature = "alloc"))]
impl Write for alloc::vec::Vec<u8> {
    #[inline]
    fn write_str(&mut self, string: &str) -> Result<(), Error> {
        self.extend_from_slice(string.as_bytes());
        Ok(())
    }
}

impl<W: Write> Write for &mut W {
    fn write_str(&mut self, string: &str) -> Result<(), Error> {
        (**self).write_str(string)
    }

    #[inline]
    fn write_char(&mut self, char: char) -> Result<(), Error> {
        (**self).write_char(char)
    }
}

/// An adapter for using `std::io::Write` implementations with `fmt::Write`.
///
/// This is useful when one wants to format a datetime or span directly
/// to something with a `std::io::Write` trait implementation but not a
/// `fmt::Write` implementation.
///
/// # Example
///
/// ```no_run
/// use std::{fs::File, io::{BufWriter, Write}, path::Path};
///
/// use jiff::{civil::date, fmt::{StdIoWrite, temporal::DateTimePrinter}};
///
/// let zdt = date(2024, 6, 15).at(7, 0, 0, 0).in_tz("America/New_York")?;
///
/// let path = Path::new("/tmp/output");
/// let mut file = BufWriter::new(File::create(path)?);
/// DateTimePrinter::new().print_zoned(&zdt, StdIoWrite(&mut file)).unwrap();
/// file.flush()?;
/// assert_eq!(
///     std::fs::read_to_string(path)?,
///     "2024-06-15T07:00:00-04:00[America/New_York]",
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg(feature = "std")]
#[derive(Clone, Debug)]
pub struct StdIoWrite<W>(pub W);

#[cfg(feature = "std")]
impl<W: std::io::Write> Write for StdIoWrite<W> {
    #[inline]
    fn write_str(&mut self, string: &str) -> Result<(), Error> {
        self.0.write_all(string.as_bytes()).map_err(Error::adhoc)
    }
}

/// An adapter for using `std::fmt::Write` implementations with `fmt::Write`.
///
/// This is useful when one wants to format a datetime or span directly
/// to something with a `std::fmt::Write` trait implementation but not a
/// `fmt::Write` implementation.
///
/// (Despite using `Std` in this name, this type is available in `core`-only
/// configurations.)
///
/// # Example
///
/// This example shows the `std::fmt::Display` trait implementation for
/// [`civil::DateTime`](crate::civil::DateTime) (but using a wrapper type).
///
/// ```
/// use jiff::{civil::DateTime, fmt::{temporal::DateTimePrinter, StdFmtWrite}};
///
/// struct MyDateTime(DateTime);
///
/// impl std::fmt::Display for MyDateTime {
///     fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
///
///         static P: DateTimePrinter = DateTimePrinter::new();
///         P.print_datetime(&self.0, StdFmtWrite(f))
///             .map_err(|_| std::fmt::Error)
///     }
/// }
///
/// let dt = MyDateTime(DateTime::constant(2024, 6, 15, 17, 30, 0, 0));
/// assert_eq!(dt.to_string(), "2024-06-15T17:30:00");
/// ```
#[derive(Clone, Debug)]
pub struct StdFmtWrite<W>(pub W);

impl<W: core::fmt::Write> Write for StdFmtWrite<W> {
    #[inline]
    fn write_str(&mut self, string: &str) -> Result<(), Error> {
        self.0
            .write_str(string)
            .map_err(|_| err!("an error occurred when formatting an argument"))
    }
}

impl<W: Write> core::fmt::Write for StdFmtWrite<W> {
    #[inline]
    fn write_str(&mut self, string: &str) -> Result<(), core::fmt::Error> {
        self.0.write_str(string).map_err(|_| core::fmt::Error)
    }
}

/// An extension trait to `Write` that provides crate internal routines.
///
/// These routines aren't exposed because they make use of crate internal
/// types. Those types could perhaps be exposed if there was strong demand,
/// but I'm skeptical.
trait WriteExt: Write {
    /// Write the given number as a decimal using ASCII digits to this buffer.
    /// The given formatter controls how the decimal is formatted.
    #[inline]
    fn write_int(
        &mut self,
        formatter: &DecimalFormatter,
        n: impl Into<i64>,
    ) -> Result<(), Error> {
        self.write_decimal(&Decimal::new(formatter, n.into()))
    }

    /// Write the given fractional number using ASCII digits to this buffer.
    /// The given formatter controls how the fractional number is formatted.
    #[inline]
    fn write_fraction(
        &mut self,
        formatter: &FractionalFormatter,
        n: impl Into<i64>,
    ) -> Result<(), Error> {
        self.write_fractional(&Fractional::new(formatter, n.into()))
    }

    /// Write the given decimal number to this buffer.
    #[inline]
    fn write_decimal(&mut self, decimal: &Decimal) -> Result<(), Error> {
        self.write_str(decimal.as_str())
    }

    /// Write the given fractional number to this buffer.
    #[inline]
    fn write_fractional(
        &mut self,
        fractional: &Fractional,
    ) -> Result<(), Error> {
        self.write_str(fractional.as_str())
    }
}

impl<W: Write> WriteExt for W {}
