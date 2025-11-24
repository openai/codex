// This is a part of Chrono.
// See README.md and LICENSE.txt for details.

//! Formatting (and parsing) utilities for date and time.
//!
//! This module provides the common types and routines to implement,
//! for example, [`DateTime::format`](../struct.DateTime.html#method.format) or
//! [`DateTime::parse_from_str`](../struct.DateTime.html#method.parse_from_str) methods.
//! For most cases you should use these high-level interfaces.
//!
//! Internally the formatting and parsing shares the same abstract **formatting items**,
//! which are just an [`Iterator`](https://doc.rust-lang.org/std/iter/trait.Iterator.html) of
//! the [`Item`](./enum.Item.html) type.
//! They are generated from more readable **format strings**;
//! currently Chrono supports a built-in syntax closely resembling
//! C's `strftime` format. The available options can be found [here](./strftime/index.html).
//!
//! # Example
//! ```
//! # #[cfg(feature = "alloc")] {
//! use chrono::{NaiveDateTime, TimeZone, Utc};
//!
//! let date_time = Utc.with_ymd_and_hms(2020, 11, 10, 0, 1, 32).unwrap();
//!
//! let formatted = format!("{}", date_time.format("%Y-%m-%d %H:%M:%S"));
//! assert_eq!(formatted, "2020-11-10 00:01:32");
//!
//! let parsed = NaiveDateTime::parse_from_str(&formatted, "%Y-%m-%d %H:%M:%S")?.and_utc();
//! assert_eq!(parsed, date_time);
//! # }
//! # Ok::<(), chrono::ParseError>(())
//! ```

#[cfg(all(feature = "alloc", not(feature = "std"), not(test)))]
use alloc::boxed::Box;
#[cfg(all(feature = "core-error", not(feature = "std")))]
use core::error::Error;
use core::fmt;
use core::str::FromStr;
#[cfg(feature = "std")]
use std::error::Error;

use crate::{Month, ParseMonthError, ParseWeekdayError, Weekday};

mod formatting;
mod parsed;

// due to the size of parsing routines, they are in separate modules.
mod parse;
pub(crate) mod scan;

pub mod strftime;

#[allow(unused)]
// TODO: remove '#[allow(unused)]' once we use this module for parsing or something else that does
// not require `alloc`.
pub(crate) mod locales;

pub use formatting::SecondsFormat;
pub(crate) use formatting::write_hundreds;
#[cfg(feature = "alloc")]
pub(crate) use formatting::write_rfc2822;
#[cfg(any(feature = "alloc", feature = "serde"))]
pub(crate) use formatting::write_rfc3339;
#[cfg(feature = "alloc")]
#[allow(deprecated)]
pub use formatting::{DelayedFormat, format, format_item};
#[cfg(feature = "unstable-locales")]
pub use locales::Locale;
pub(crate) use parse::parse_rfc3339;
pub use parse::{parse, parse_and_remainder};
pub use parsed::Parsed;
pub use strftime::StrftimeItems;

/// An uninhabited type used for `InternalNumeric` and `InternalFixed` below.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Void {}

/// Padding characters for numeric items.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum Pad {
    /// No padding.
    None,
    /// Zero (`0`) padding.
    Zero,
    /// Space padding.
    Space,
}

/// Numeric item types.
/// They have associated formatting width (FW) and parsing width (PW).
///
/// The **formatting width** is the minimal width to be formatted.
/// If the number is too short, and the padding is not [`Pad::None`](./enum.Pad.html#variant.None),
/// then it is left-padded.
/// If the number is too long or (in some cases) negative, it is printed as is.
///
/// The **parsing width** is the maximal width to be scanned.
/// The parser only tries to consume from one to given number of digits (greedily).
/// It also trims the preceding whitespace if any.
/// It cannot parse the negative number, so some date and time cannot be formatted then
/// parsed with the same formatting items.
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum Numeric {
    /// Full Gregorian year (FW=4, PW=∞).
    /// May accept years before 1 BCE or after 9999 CE, given an initial sign (+/-).
    Year,
    /// Gregorian year divided by 100 (century number; FW=PW=2). Implies the non-negative year.
    YearDiv100,
    /// Gregorian year modulo 100 (FW=PW=2). Cannot be negative.
    YearMod100,
    /// Year in the ISO week date (FW=4, PW=∞).
    /// May accept years before 1 BCE or after 9999 CE, given an initial sign.
    IsoYear,
    /// Year in the ISO week date, divided by 100 (FW=PW=2). Implies the non-negative year.
    IsoYearDiv100,
    /// Year in the ISO week date, modulo 100 (FW=PW=2). Cannot be negative.
    IsoYearMod100,
    /// Quarter (FW=PW=1).
    Quarter,
    /// Month (FW=PW=2).
    Month,
    /// Day of the month (FW=PW=2).
    Day,
    /// Week number, where the week 1 starts at the first Sunday of January (FW=PW=2).
    WeekFromSun,
    /// Week number, where the week 1 starts at the first Monday of January (FW=PW=2).
    WeekFromMon,
    /// Week number in the ISO week date (FW=PW=2).
    IsoWeek,
    /// Day of the week, where Sunday = 0 and Saturday = 6 (FW=PW=1).
    NumDaysFromSun,
    /// Day of the week, where Monday = 1 and Sunday = 7 (FW=PW=1).
    WeekdayFromMon,
    /// Day of the year (FW=PW=3).
    Ordinal,
    /// Hour number in the 24-hour clocks (FW=PW=2).
    Hour,
    /// Hour number in the 12-hour clocks (FW=PW=2).
    Hour12,
    /// The number of minutes since the last whole hour (FW=PW=2).
    Minute,
    /// The number of seconds since the last whole minute (FW=PW=2).
    Second,
    /// The number of nanoseconds since the last whole second (FW=PW=9).
    /// Note that this is *not* left-aligned;
    /// see also [`Fixed::Nanosecond`](./enum.Fixed.html#variant.Nanosecond).
    Nanosecond,
    /// The number of non-leap seconds since the midnight UTC on January 1, 1970 (FW=1, PW=∞).
    /// For formatting, it assumes UTC upon the absence of time zone offset.
    Timestamp,

    /// Internal uses only.
    ///
    /// This item exists so that one can add additional internal-only formatting
    /// without breaking major compatibility (as enum variants cannot be selectively private).
    Internal(InternalNumeric),
}

/// An opaque type representing numeric item types for internal uses only.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct InternalNumeric {
    _dummy: Void,
}

impl fmt::Debug for InternalNumeric {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<InternalNumeric>")
    }
}

/// Fixed-format item types.
///
/// They have their own rules of formatting and parsing.
/// Otherwise noted, they print in the specified cases but parse case-insensitively.
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum Fixed {
    /// Abbreviated month names.
    ///
    /// Prints a three-letter-long name in the title case, reads the same name in any case.
    ShortMonthName,
    /// Full month names.
    ///
    /// Prints a full name in the title case, reads either a short or full name in any case.
    LongMonthName,
    /// Abbreviated day of the week names.
    ///
    /// Prints a three-letter-long name in the title case, reads the same name in any case.
    ShortWeekdayName,
    /// Full day of the week names.
    ///
    /// Prints a full name in the title case, reads either a short or full name in any case.
    LongWeekdayName,
    /// AM/PM.
    ///
    /// Prints in lower case, reads in any case.
    LowerAmPm,
    /// AM/PM.
    ///
    /// Prints in upper case, reads in any case.
    UpperAmPm,
    /// An optional dot plus one or more digits for left-aligned nanoseconds.
    /// May print nothing, 3, 6 or 9 digits according to the available accuracy.
    /// See also [`Numeric::Nanosecond`](./enum.Numeric.html#variant.Nanosecond).
    Nanosecond,
    /// Same as [`Nanosecond`](#variant.Nanosecond) but the accuracy is fixed to 3.
    Nanosecond3,
    /// Same as [`Nanosecond`](#variant.Nanosecond) but the accuracy is fixed to 6.
    Nanosecond6,
    /// Same as [`Nanosecond`](#variant.Nanosecond) but the accuracy is fixed to 9.
    Nanosecond9,
    /// Timezone name.
    ///
    /// It does not support parsing, its use in the parser is an immediate failure.
    TimezoneName,
    /// Offset from the local time to UTC (`+09:00` or `-04:00` or `+00:00`).
    ///
    /// In the parser, the colon can be omitted and/or surrounded with any amount of whitespace.
    /// The offset is limited from `-24:00` to `+24:00`,
    /// which is the same as [`FixedOffset`](../offset/struct.FixedOffset.html)'s range.
    TimezoneOffsetColon,
    /// Offset from the local time to UTC with seconds (`+09:00:00` or `-04:00:00` or `+00:00:00`).
    ///
    /// In the parser, the colon can be omitted and/or surrounded with any amount of whitespace.
    /// The offset is limited from `-24:00:00` to `+24:00:00`,
    /// which is the same as [`FixedOffset`](../offset/struct.FixedOffset.html)'s range.
    TimezoneOffsetDoubleColon,
    /// Offset from the local time to UTC without minutes (`+09` or `-04` or `+00`).
    ///
    /// In the parser, the colon can be omitted and/or surrounded with any amount of whitespace.
    /// The offset is limited from `-24` to `+24`,
    /// which is the same as [`FixedOffset`](../offset/struct.FixedOffset.html)'s range.
    TimezoneOffsetTripleColon,
    /// Offset from the local time to UTC (`+09:00` or `-04:00` or `Z`).
    ///
    /// In the parser, the colon can be omitted and/or surrounded with any amount of whitespace,
    /// and `Z` can be either in upper case or in lower case.
    /// The offset is limited from `-24:00` to `+24:00`,
    /// which is the same as [`FixedOffset`](../offset/struct.FixedOffset.html)'s range.
    TimezoneOffsetColonZ,
    /// Same as [`TimezoneOffsetColon`](#variant.TimezoneOffsetColon) but prints no colon.
    /// Parsing allows an optional colon.
    TimezoneOffset,
    /// Same as [`TimezoneOffsetColonZ`](#variant.TimezoneOffsetColonZ) but prints no colon.
    /// Parsing allows an optional colon.
    TimezoneOffsetZ,
    /// RFC 2822 date and time syntax. Commonly used for email and MIME date and time.
    RFC2822,
    /// RFC 3339 & ISO 8601 date and time syntax.
    RFC3339,

    /// Internal uses only.
    ///
    /// This item exists so that one can add additional internal-only formatting
    /// without breaking major compatibility (as enum variants cannot be selectively private).
    Internal(InternalFixed),
}

/// An opaque type representing fixed-format item types for internal uses only.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InternalFixed {
    val: InternalInternal,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum InternalInternal {
    /// Same as [`TimezoneOffsetColonZ`](#variant.TimezoneOffsetColonZ), but
    /// allows missing minutes (per [ISO 8601][iso8601]).
    ///
    /// # Panics
    ///
    /// If you try to use this for printing.
    ///
    /// [iso8601]: https://en.wikipedia.org/wiki/ISO_8601#Time_offsets_from_UTC
    TimezoneOffsetPermissive,
    /// Same as [`Nanosecond`](#variant.Nanosecond) but the accuracy is fixed to 3 and there is no leading dot.
    Nanosecond3NoDot,
    /// Same as [`Nanosecond`](#variant.Nanosecond) but the accuracy is fixed to 6 and there is no leading dot.
    Nanosecond6NoDot,
    /// Same as [`Nanosecond`](#variant.Nanosecond) but the accuracy is fixed to 9 and there is no leading dot.
    Nanosecond9NoDot,
}

/// Type for specifying the format of UTC offsets.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct OffsetFormat {
    /// See `OffsetPrecision`.
    pub precision: OffsetPrecision,
    /// Separator between hours, minutes and seconds.
    pub colons: Colons,
    /// Represent `+00:00` as `Z`.
    pub allow_zulu: bool,
    /// Pad the hour value to two digits.
    pub padding: Pad,
}

/// The precision of an offset from UTC formatting item.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum OffsetPrecision {
    /// Format offset from UTC as only hours. Not recommended, it is not uncommon for timezones to
    /// have an offset of 30 minutes, 15 minutes, etc.
    /// Any minutes and seconds get truncated.
    Hours,
    /// Format offset from UTC as hours and minutes.
    /// Any seconds will be rounded to the nearest minute.
    Minutes,
    /// Format offset from UTC as hours, minutes and seconds.
    Seconds,
    /// Format offset from UTC as hours, and optionally with minutes.
    /// Any seconds will be rounded to the nearest minute.
    OptionalMinutes,
    /// Format offset from UTC as hours and minutes, and optionally seconds.
    OptionalSeconds,
    /// Format offset from UTC as hours and optionally minutes and seconds.
    OptionalMinutesAndSeconds,
}

/// The separator between hours and minutes in an offset.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Colons {
    /// No separator
    None,
    /// Colon (`:`) as separator
    Colon,
    /// No separator when formatting, colon allowed when parsing.
    Maybe,
}

/// A single formatting item. This is used for both formatting and parsing.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub enum Item<'a> {
    /// A literally printed and parsed text.
    Literal(&'a str),
    /// Same as `Literal` but with the string owned by the item.
    #[cfg(feature = "alloc")]
    OwnedLiteral(Box<str>),
    /// Whitespace. Prints literally but reads zero or more whitespace.
    Space(&'a str),
    /// Same as `Space` but with the string owned by the item.
    #[cfg(feature = "alloc")]
    OwnedSpace(Box<str>),
    /// Numeric item. Can be optionally padded to the maximal length (if any) when formatting;
    /// the parser simply ignores any padded whitespace and zeroes.
    Numeric(Numeric, Pad),
    /// Fixed-format item.
    Fixed(Fixed),
    /// Issues a formatting error. Used to signal an invalid format string.
    Error,
}

const fn num(numeric: Numeric) -> Item<'static> {
    Item::Numeric(numeric, Pad::None)
}

const fn num0(numeric: Numeric) -> Item<'static> {
    Item::Numeric(numeric, Pad::Zero)
}

const fn nums(numeric: Numeric) -> Item<'static> {
    Item::Numeric(numeric, Pad::Space)
}

const fn fixed(fixed: Fixed) -> Item<'static> {
    Item::Fixed(fixed)
}

const fn internal_fixed(val: InternalInternal) -> Item<'static> {
    Item::Fixed(Fixed::Internal(InternalFixed { val }))
}

impl Item<'_> {
    /// Convert items that contain a reference to the format string into an owned variant.
    #[cfg(any(feature = "alloc", feature = "std"))]
    pub fn to_owned(self) -> Item<'static> {
        match self {
            Item::Literal(s) => Item::OwnedLiteral(Box::from(s)),
            Item::Space(s) => Item::OwnedSpace(Box::from(s)),
            Item::Numeric(n, p) => Item::Numeric(n, p),
            Item::Fixed(f) => Item::Fixed(f),
            Item::OwnedLiteral(l) => Item::OwnedLiteral(l),
            Item::OwnedSpace(s) => Item::OwnedSpace(s),
            Item::Error => Item::Error,
        }
    }
}

/// An error from the `parse` function.
#[derive(Debug, Clone, PartialEq, Eq, Copy, Hash)]
pub struct ParseError(ParseErrorKind);

impl ParseError {
    /// The category of parse error
    pub const fn kind(&self) -> ParseErrorKind {
        self.0
    }
}

/// The category of parse error
#[allow(clippy::manual_non_exhaustive)]
#[derive(Debug, Clone, PartialEq, Eq, Copy, Hash)]
pub enum ParseErrorKind {
    /// Given field is out of permitted range.
    OutOfRange,

    /// There is no possible date and time value with given set of fields.
    ///
    /// This does not include the out-of-range conditions, which are trivially invalid.
    /// It includes the case that there are one or more fields that are inconsistent to each other.
    Impossible,

    /// Given set of fields is not enough to make a requested date and time value.
    ///
    /// Note that there *may* be a case that given fields constrain the possible values so much
    /// that there is a unique possible value. Chrono only tries to be correct for
    /// most useful sets of fields however, as such constraint solving can be expensive.
    NotEnough,

    /// The input string has some invalid character sequence for given formatting items.
    Invalid,

    /// The input string has been prematurely ended.
    TooShort,

    /// All formatting items have been read but there is a remaining input.
    TooLong,

    /// There was an error on the formatting string, or there were non-supported formatting items.
    BadFormat,

    // TODO: Change this to `#[non_exhaustive]` (on the enum) with the next breaking release.
    #[doc(hidden)]
    __Nonexhaustive,
}

/// Same as `Result<T, ParseError>`.
pub type ParseResult<T> = Result<T, ParseError>;

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            ParseErrorKind::OutOfRange => write!(f, "input is out of range"),
            ParseErrorKind::Impossible => write!(f, "no possible date and time matching input"),
            ParseErrorKind::NotEnough => write!(f, "input is not enough for unique date and time"),
            ParseErrorKind::Invalid => write!(f, "input contains invalid characters"),
            ParseErrorKind::TooShort => write!(f, "premature end of input"),
            ParseErrorKind::TooLong => write!(f, "trailing input"),
            ParseErrorKind::BadFormat => write!(f, "bad or unsupported format string"),
            _ => unreachable!(),
        }
    }
}

#[cfg(any(feature = "core-error", feature = "std"))]
impl Error for ParseError {
    #[allow(deprecated)]
    fn description(&self) -> &str {
        "parser error, see to_string() for details"
    }
}

// to be used in this module and submodules
pub(crate) const OUT_OF_RANGE: ParseError = ParseError(ParseErrorKind::OutOfRange);
const IMPOSSIBLE: ParseError = ParseError(ParseErrorKind::Impossible);
const NOT_ENOUGH: ParseError = ParseError(ParseErrorKind::NotEnough);
const INVALID: ParseError = ParseError(ParseErrorKind::Invalid);
const TOO_SHORT: ParseError = ParseError(ParseErrorKind::TooShort);
pub(crate) const TOO_LONG: ParseError = ParseError(ParseErrorKind::TooLong);
const BAD_FORMAT: ParseError = ParseError(ParseErrorKind::BadFormat);

// this implementation is here only because we need some private code from `scan`

/// Parsing a `str` into a `Weekday` uses the format [`%A`](./format/strftime/index.html).
///
/// # Example
///
/// ```
/// use chrono::Weekday;
///
/// assert_eq!("Sunday".parse::<Weekday>(), Ok(Weekday::Sun));
/// assert!("any day".parse::<Weekday>().is_err());
/// ```
///
/// The parsing is case-insensitive.
///
/// ```
/// # use chrono::Weekday;
/// assert_eq!("mON".parse::<Weekday>(), Ok(Weekday::Mon));
/// ```
///
/// Only the shortest form (e.g. `sun`) and the longest form (e.g. `sunday`) is accepted.
///
/// ```
/// # use chrono::Weekday;
/// assert!("thurs".parse::<Weekday>().is_err());
/// ```
impl FromStr for Weekday {
    type Err = ParseWeekdayError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(("", w)) = scan::short_or_long_weekday(s) {
            Ok(w)
        } else {
            Err(ParseWeekdayError { _dummy: () })
        }
    }
}

/// Parsing a `str` into a `Month` uses the format [`%B`](./format/strftime/index.html).
///
/// # Example
///
/// ```
/// use chrono::Month;
///
/// assert_eq!("January".parse::<Month>(), Ok(Month::January));
/// assert!("any day".parse::<Month>().is_err());
/// ```
///
/// The parsing is case-insensitive.
///
/// ```
/// # use chrono::Month;
/// assert_eq!("fEbruARy".parse::<Month>(), Ok(Month::February));
/// ```
///
/// Only the shortest form (e.g. `jan`) and the longest form (e.g. `january`) is accepted.
///
/// ```
/// # use chrono::Month;
/// assert!("septem".parse::<Month>().is_err());
/// assert!("Augustin".parse::<Month>().is_err());
/// ```
impl FromStr for Month {
    type Err = ParseMonthError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(("", w)) = scan::short_or_long_month0(s) {
            match w {
                0 => Ok(Month::January),
                1 => Ok(Month::February),
                2 => Ok(Month::March),
                3 => Ok(Month::April),
                4 => Ok(Month::May),
                5 => Ok(Month::June),
                6 => Ok(Month::July),
                7 => Ok(Month::August),
                8 => Ok(Month::September),
                9 => Ok(Month::October),
                10 => Ok(Month::November),
                11 => Ok(Month::December),
                _ => Err(ParseMonthError { _dummy: () }),
            }
        } else {
            Err(ParseMonthError { _dummy: () })
        }
    }
}
