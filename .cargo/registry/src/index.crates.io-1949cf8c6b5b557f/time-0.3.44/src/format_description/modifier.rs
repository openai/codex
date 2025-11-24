//! Various modifiers for components.

use core::num::NonZero;

/// Day of the month.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Day {
    /// The padding to obtain the minimum width.
    pub padding: Padding,
}

/// The representation of a month.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonthRepr {
    /// The number of the month (January is 1, December is 12).
    Numerical,
    /// The long form of the month name (e.g. "January").
    Long,
    /// The short form of the month name (e.g. "Jan").
    Short,
}

/// Month of the year.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Month {
    /// The padding to obtain the minimum width.
    pub padding: Padding,
    /// What form of representation should be used?
    pub repr: MonthRepr,
    /// Is the value case sensitive when parsing?
    pub case_sensitive: bool,
}

/// Ordinal day of the year.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ordinal {
    /// The padding to obtain the minimum width.
    pub padding: Padding,
}

/// The representation used for the day of the week.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeekdayRepr {
    /// The short form of the weekday (e.g. "Mon").
    Short,
    /// The long form of the weekday (e.g. "Monday").
    Long,
    /// A numerical representation using Sunday as the first day of the week.
    ///
    /// Sunday is either 0 or 1, depending on the other modifier's value.
    Sunday,
    /// A numerical representation using Monday as the first day of the week.
    ///
    /// Monday is either 0 or 1, depending on the other modifier's value.
    Monday,
}

/// Day of the week.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weekday {
    /// What form of representation should be used?
    pub repr: WeekdayRepr,
    /// When using a numerical representation, should it be zero or one-indexed?
    pub one_indexed: bool,
    /// Is the value case sensitive when parsing?
    pub case_sensitive: bool,
}

/// The representation used for the week number.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeekNumberRepr {
    /// Week 1 is the week that contains January 4.
    Iso,
    /// Week 1 begins on the first Sunday of the calendar year.
    Sunday,
    /// Week 1 begins on the first Monday of the calendar year.
    Monday,
}

/// Week within the year.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeekNumber {
    /// The padding to obtain the minimum width.
    pub padding: Padding,
    /// What kind of representation should be used?
    pub repr: WeekNumberRepr,
}

/// The representation used for a year value.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YearRepr {
    /// The full value of the year.
    Full,
    /// All digits except the last two. Includes the sign, if any.
    Century,
    /// Only the last two digits of the year.
    LastTwo,
}

/// The range of years that are supported.
///
/// This modifier has no effect when the year repr is [`LastTwo`](YearRepr::LastTwo).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YearRange {
    /// Years between -9999 and 9999 are supported.
    Standard,
    /// Years between -999_999 and 999_999 are supported, with the sign being required if the year
    /// contains more than four digits.
    ///
    /// If the `large-dates` feature is not enabled, this variant is equivalent to `Standard`.
    Extended,
}

/// Year of the date.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Year {
    /// The padding to obtain the minimum width.
    pub padding: Padding,
    /// What kind of representation should be used?
    pub repr: YearRepr,
    /// What range of years is supported?
    pub range: YearRange,
    /// Whether the value is based on the ISO week number or the Gregorian calendar.
    pub iso_week_based: bool,
    /// Whether the `+` sign is present when a positive year contains fewer than five digits.
    pub sign_is_mandatory: bool,
}

/// Hour of the day.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hour {
    /// The padding to obtain the minimum width.
    pub padding: Padding,
    /// Is the hour displayed using a 12 or 24-hour clock?
    pub is_12_hour_clock: bool,
}

/// Minute within the hour.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Minute {
    /// The padding to obtain the minimum width.
    pub padding: Padding,
}

/// AM/PM part of the time.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Period {
    /// Is the period uppercase or lowercase?
    pub is_uppercase: bool,
    /// Is the value case sensitive when parsing?
    ///
    /// Note that when `false`, the `is_uppercase` field has no effect on parsing behavior.
    pub case_sensitive: bool,
}

/// Second within the minute.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Second {
    /// The padding to obtain the minimum width.
    pub padding: Padding,
}

/// The number of digits present in a subsecond representation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsecondDigits {
    /// Exactly one digit.
    One,
    /// Exactly two digits.
    Two,
    /// Exactly three digits.
    Three,
    /// Exactly four digits.
    Four,
    /// Exactly five digits.
    Five,
    /// Exactly six digits.
    Six,
    /// Exactly seven digits.
    Seven,
    /// Exactly eight digits.
    Eight,
    /// Exactly nine digits.
    Nine,
    /// Any number of digits (up to nine) that is at least one. When formatting, the minimum digits
    /// necessary will be used.
    OneOrMore,
}

/// Subsecond within the second.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subsecond {
    /// How many digits are present in the component?
    pub digits: SubsecondDigits,
}

/// Hour of the UTC offset.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffsetHour {
    /// Whether the `+` sign is present on positive values.
    pub sign_is_mandatory: bool,
    /// The padding to obtain the minimum width.
    pub padding: Padding,
}

/// Minute within the hour of the UTC offset.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffsetMinute {
    /// The padding to obtain the minimum width.
    pub padding: Padding,
}

/// Second within the minute of the UTC offset.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffsetSecond {
    /// The padding to obtain the minimum width.
    pub padding: Padding,
}

/// Type of padding to ensure a minimum width.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Padding {
    /// A space character (` `) should be used as padding.
    Space,
    /// A zero character (`0`) should be used as padding.
    Zero,
    /// There is no padding. This can result in a width below the otherwise minimum number of
    /// characters.
    None,
}

/// Ignore some number of bytes.
///
/// This has no effect when formatting.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ignore {
    /// The number of bytes to ignore.
    pub count: NonZero<u16>,
}

// Needed as `Default` is deliberately not implemented for `Ignore`. The number of bytes to ignore
// must be explicitly provided.
impl Ignore {
    /// Create an instance of `Ignore` with the provided number of bytes to ignore.
    #[inline]
    pub const fn count(count: NonZero<u16>) -> Self {
        Self { count }
    }
}

/// The precision of a Unix timestamp.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnixTimestampPrecision {
    /// Seconds since the Unix epoch.
    Second,
    /// Milliseconds since the Unix epoch.
    Millisecond,
    /// Microseconds since the Unix epoch.
    Microsecond,
    /// Nanoseconds since the Unix epoch.
    Nanosecond,
}

/// A Unix timestamp.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnixTimestamp {
    /// The precision of the timestamp.
    pub precision: UnixTimestampPrecision,
    /// Whether the `+` sign must be present for a non-negative timestamp.
    pub sign_is_mandatory: bool,
}

/// The end of input.
///
/// There is currently not customization for this modifier.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct End;

/// Generate the provided code if and only if `pub` is present.
macro_rules! if_pub {
    (pub $(#[$attr:meta])*; $($x:tt)*) => {
        $(#[$attr])*
        ///
        /// This function exists since [`Default::default()`] cannot be used in a `const` context.
        /// It may be removed once that becomes possible. As the [`Default`] trait is in the
        /// prelude, removing this function in the future will not cause any resolution failures for
        /// the overwhelming majority of users; only users who use `#![no_implicit_prelude]` will be
        /// affected. As such it will not be considered a breaking change.
        $($x)*
    };
    ($($_:tt)*) => {};
}

/// Implement `Default` for the given type. This also generates an inherent implementation of a
/// `default` method that is `const fn`, permitting the default value to be used in const contexts.
// Every modifier should use this macro rather than a derived `Default`.
macro_rules! impl_const_default {
    ($($(#[$doc:meta])* $(@$pub:ident)? $type:ty => $default:expr;)*) => {$(
        impl $type {
            if_pub! {
                $($pub)?
                $(#[$doc])*;
                #[inline]
                pub const fn default() -> Self {
                    $default
                }
            }
        }

        $(#[$doc])*
        impl Default for $type {
            #[inline]
            fn default() -> Self {
                $default
            }
        }
    )*};
}

impl_const_default! {
    /// Creates a modifier that indicates the value is [padded with zeroes](Padding::Zero).
    @pub Day => Self { padding: Padding::Zero };
    /// Creates a modifier that indicates the value uses the
    /// [`Numerical`](Self::Numerical) representation.
    MonthRepr => Self::Numerical;
    /// Creates an instance of this type that indicates the value uses the
    /// [`Numerical`](MonthRepr::Numerical) representation, is [padded with zeroes](Padding::Zero),
    /// and is case-sensitive when parsing.
    @pub Month => Self {
        padding: Padding::Zero,
        repr: MonthRepr::Numerical,
        case_sensitive: true,
    };
    /// Creates a modifier that indicates the value is [padded with zeroes](Padding::Zero).
    @pub Ordinal => Self { padding: Padding::Zero };
    /// Creates a modifier that indicates the value uses the [`Long`](Self::Long) representation.
    WeekdayRepr => Self::Long;
    /// Creates a modifier that indicates the value uses the [`Long`](WeekdayRepr::Long)
    /// representation and is case-sensitive when parsing. If the representation is changed to a
    /// numerical one, the instance defaults to one-based indexing.
    @pub Weekday => Self {
        repr: WeekdayRepr::Long,
        one_indexed: true,
        case_sensitive: true,
    };
    /// Creates a modifier that indicates that the value uses the [`Iso`](Self::Iso) representation.
    WeekNumberRepr => Self::Iso;
    /// Creates a modifier that indicates that the value is [padded with zeroes](Padding::Zero)
            /// and uses the [`Iso`](WeekNumberRepr::Iso) representation.
    @pub WeekNumber => Self {
        padding: Padding::Zero,
        repr: WeekNumberRepr::Iso,
    };
    /// Creates a modifier that indicates the value uses the [`Full`](Self::Full) representation.
    YearRepr => Self::Full;
    /// Creates a modifier that indicates the value uses the [`Extended`](Self::Extended) range.
    YearRange => Self::Extended;
    /// Creates a modifier that indicates the value uses the [`Full`](YearRepr::Full)
    /// representation, is [padded with zeroes](Padding::Zero), uses the Gregorian calendar as its
    /// base, and only includes the year's sign if necessary.
    @pub Year => Self {
        padding: Padding::Zero,
        repr: YearRepr::Full,
        range: YearRange::Extended,
        iso_week_based: false,
        sign_is_mandatory: false,
    };
    /// Creates a modifier that indicates the value is [padded with zeroes](Padding::Zero) and
    /// has the 24-hour representation.
    @pub Hour => Self {
        padding: Padding::Zero,
        is_12_hour_clock: false,
    };
    /// Creates a modifier that indicates the value is [padded with zeroes](Padding::Zero).
    @pub Minute => Self { padding: Padding::Zero };
    /// Creates a modifier that indicates the value uses the upper-case representation and is
    /// case-sensitive when parsing.
    @pub Period => Self {
        is_uppercase: true,
        case_sensitive: true,
    };
    /// Creates a modifier that indicates the value is [padded with zeroes](Padding::Zero).
    @pub Second => Self { padding: Padding::Zero };
    /// Creates a modifier that indicates the stringified value contains [one or more
    /// digits](Self::OneOrMore).
    SubsecondDigits => Self::OneOrMore;
    /// Creates a modifier that indicates the stringified value contains [one or more
    /// digits](SubsecondDigits::OneOrMore).
    @pub Subsecond => Self { digits: SubsecondDigits::OneOrMore };
    /// Creates a modifier that indicates the value only uses a sign for negative values and is
    /// [padded with zeroes](Padding::Zero).
    @pub OffsetHour => Self {
        sign_is_mandatory: false,
        padding: Padding::Zero,
    };
    /// Creates a modifier that indicates the value is [padded with zeroes](Padding::Zero).
    @pub OffsetMinute => Self { padding: Padding::Zero };
    /// Creates a modifier that indicates the value is [padded with zeroes](Padding::Zero).
    @pub OffsetSecond => Self { padding: Padding::Zero };
    /// Creates a modifier that indicates the value is [padded with zeroes](Self::Zero).
    Padding => Self::Zero;
    /// Creates a modifier that indicates the value represents the [number of seconds](Self::Second)
    /// since the Unix epoch.
    UnixTimestampPrecision => Self::Second;
    /// Creates a modifier that indicates the value represents the [number of
    /// seconds](UnixTimestampPrecision::Second) since the Unix epoch. The sign is not mandatory.
    @pub UnixTimestamp => Self {
        precision: UnixTimestampPrecision::Second,
        sign_is_mandatory: false,
    };
    /// Creates a modifier used to represent the end of input.
    @pub End => End;
}
