//! Formatting for various types.

pub(crate) mod formattable;
mod iso8601;

use core::num::NonZero;
use std::io;

use num_conv::prelude::*;

pub use self::formattable::Formattable;
use crate::convert::*;
use crate::ext::DigitCount;
use crate::format_description::{modifier, Component};
use crate::{error, Date, OffsetDateTime, Time, UtcOffset};

const MONTH_NAMES: [&[u8]; 12] = [
    b"January",
    b"February",
    b"March",
    b"April",
    b"May",
    b"June",
    b"July",
    b"August",
    b"September",
    b"October",
    b"November",
    b"December",
];

const WEEKDAY_NAMES: [&[u8]; 7] = [
    b"Monday",
    b"Tuesday",
    b"Wednesday",
    b"Thursday",
    b"Friday",
    b"Saturday",
    b"Sunday",
];

/// Write all bytes to the output, returning the number of bytes written.
#[inline]
pub(crate) fn write(output: &mut (impl io::Write + ?Sized), bytes: &[u8]) -> io::Result<usize> {
    output.write_all(bytes)?;
    Ok(bytes.len())
}

/// If `pred` is true, write all bytes to the output, returning the number of bytes written.
#[inline]
pub(crate) fn write_if(
    output: &mut (impl io::Write + ?Sized),
    pred: bool,
    bytes: &[u8],
) -> io::Result<usize> {
    if pred {
        write(output, bytes)
    } else {
        Ok(0)
    }
}

/// If `pred` is true, write `true_bytes` to the output. Otherwise, write `false_bytes`.
#[inline]
pub(crate) fn write_if_else(
    output: &mut (impl io::Write + ?Sized),
    pred: bool,
    true_bytes: &[u8],
    false_bytes: &[u8],
) -> io::Result<usize> {
    write(output, if pred { true_bytes } else { false_bytes })
}

/// Helper function to obtain 10^x, guaranteeing determinism for x ≤ 9. For these cases, the
/// function optimizes to a lookup table. For x ≥ 10, it falls back to `10_f64.powi(x)`. The only
/// situation where this would occur is if the user explicitly requests such precision when
/// configuring the ISO 8601 well known format. All other possibilities max out at nine digits.
#[inline]
fn f64_10_pow_x(x: NonZero<u8>) -> f64 {
    match x.get() {
        1 => 10.,
        2 => 100.,
        3 => 1_000.,
        4 => 10_000.,
        5 => 100_000.,
        6 => 1_000_000.,
        7 => 10_000_000.,
        8 => 100_000_000.,
        9 => 1_000_000_000.,
        x => 10_f64.powi(x.cast_signed().extend()),
    }
}

/// Write the floating point number to the output, returning the number of bytes written.
///
/// This method accepts the number of digits before and after the decimal. The value will be padded
/// with zeroes to the left if necessary.
#[inline]
pub(crate) fn format_float(
    output: &mut (impl io::Write + ?Sized),
    mut value: f64,
    digits_before_decimal: u8,
    digits_after_decimal: Option<NonZero<u8>>,
) -> io::Result<usize> {
    match digits_after_decimal {
        Some(digits_after_decimal) => {
            // If the precision is less than nine digits after the decimal point, truncate the
            // value. This avoids rounding up and causing the value to exceed the maximum permitted
            // value (as in #678). If the precision is at least nine, then we don't truncate so as
            // to avoid having an off-by-one error (as in #724). The latter is necessary
            // because floating point values are inherently imprecise with decimal
            // values, so a minuscule error can be amplified easily.
            //
            // Note that this is largely an issue for second values, as for minute and hour decimals
            // the value is divided by 60 or 3,600, neither of which divide evenly into 10^x.
            //
            // While not a perfect approach, this addresses the bugs that have been reported so far
            // without being overly complex.
            if digits_after_decimal.get() < 9 {
                let trunc_num = f64_10_pow_x(digits_after_decimal);
                value = f64::trunc(value * trunc_num) / trunc_num;
            }

            let digits_after_decimal = digits_after_decimal.get().extend();
            let width = digits_before_decimal.extend::<usize>() + 1 + digits_after_decimal;
            write!(output, "{value:0>width$.digits_after_decimal$}")?;
            Ok(width)
        }
        None => {
            let value = value.trunc() as u64;
            let width = digits_before_decimal.extend();
            write!(output, "{value:0>width$}")?;
            Ok(width)
        }
    }
}

/// Format a number with the provided padding and width.
///
/// The sign must be written by the caller.
#[inline]
pub(crate) fn format_number<const WIDTH: u8>(
    output: &mut (impl io::Write + ?Sized),
    value: impl itoa::Integer + DigitCount + Copy,
    padding: modifier::Padding,
) -> Result<usize, io::Error> {
    match padding {
        modifier::Padding::Space => format_number_pad_space::<WIDTH>(output, value),
        modifier::Padding::Zero => format_number_pad_zero::<WIDTH>(output, value),
        modifier::Padding::None => format_number_pad_none(output, value),
    }
}

/// Format a number with the provided width and spaces as padding.
///
/// The sign must be written by the caller.
#[inline]
pub(crate) fn format_number_pad_space<const WIDTH: u8>(
    output: &mut (impl io::Write + ?Sized),
    value: impl itoa::Integer + DigitCount + Copy,
) -> Result<usize, io::Error> {
    let mut bytes = 0;
    for _ in 0..(WIDTH.saturating_sub(value.num_digits())) {
        bytes += write(output, b" ")?;
    }
    bytes += write(output, itoa::Buffer::new().format(value).as_bytes())?;
    Ok(bytes)
}

/// Format a number with the provided width and zeros as padding.
///
/// The sign must be written by the caller.
#[inline]
pub(crate) fn format_number_pad_zero<const WIDTH: u8>(
    output: &mut (impl io::Write + ?Sized),
    value: impl itoa::Integer + DigitCount + Copy,
) -> Result<usize, io::Error> {
    let mut bytes = 0;
    for _ in 0..(WIDTH.saturating_sub(value.num_digits())) {
        bytes += write(output, b"0")?;
    }
    bytes += write(output, itoa::Buffer::new().format(value).as_bytes())?;
    Ok(bytes)
}

/// Format a number with no padding.
///
/// If the sign is mandatory, the sign must be written by the caller.
#[inline]
pub(crate) fn format_number_pad_none(
    output: &mut (impl io::Write + ?Sized),
    value: impl itoa::Integer + Copy,
) -> Result<usize, io::Error> {
    write(output, itoa::Buffer::new().format(value).as_bytes())
}

/// Format the provided component into the designated output. An `Err` will be returned if the
/// component requires information that it does not provide or if the value cannot be output to the
/// stream.
#[inline]
pub(crate) fn format_component(
    output: &mut (impl io::Write + ?Sized),
    component: Component,
    date: Option<Date>,
    time: Option<Time>,
    offset: Option<UtcOffset>,
) -> Result<usize, error::Format> {
    use Component::*;
    Ok(match (component, date, time, offset) {
        (Day(modifier), Some(date), ..) => fmt_day(output, date, modifier)?,
        (Month(modifier), Some(date), ..) => fmt_month(output, date, modifier)?,
        (Ordinal(modifier), Some(date), ..) => fmt_ordinal(output, date, modifier)?,
        (Weekday(modifier), Some(date), ..) => fmt_weekday(output, date, modifier)?,
        (WeekNumber(modifier), Some(date), ..) => fmt_week_number(output, date, modifier)?,
        (Year(modifier), Some(date), ..) => fmt_year(output, date, modifier)?,
        (Hour(modifier), _, Some(time), _) => fmt_hour(output, time, modifier)?,
        (Minute(modifier), _, Some(time), _) => fmt_minute(output, time, modifier)?,
        (Period(modifier), _, Some(time), _) => fmt_period(output, time, modifier)?,
        (Second(modifier), _, Some(time), _) => fmt_second(output, time, modifier)?,
        (Subsecond(modifier), _, Some(time), _) => fmt_subsecond(output, time, modifier)?,
        (OffsetHour(modifier), .., Some(offset)) => fmt_offset_hour(output, offset, modifier)?,
        (OffsetMinute(modifier), .., Some(offset)) => fmt_offset_minute(output, offset, modifier)?,
        (OffsetSecond(modifier), .., Some(offset)) => fmt_offset_second(output, offset, modifier)?,
        (Ignore(_), ..) => 0,
        (UnixTimestamp(modifier), Some(date), Some(time), Some(offset)) => {
            fmt_unix_timestamp(output, date, time, offset, modifier)?
        }
        (End(modifier::End {}), ..) => 0,

        // This is functionally the same as a wildcard arm, but it will cause an error if a new
        // component is added. This is to avoid a bug where a new component, the code compiles, and
        // formatting fails.
        // Allow unreachable patterns because some branches may be fully matched above.
        #[allow(unreachable_patterns)]
        (
            Day(_) | Month(_) | Ordinal(_) | Weekday(_) | WeekNumber(_) | Year(_) | Hour(_)
            | Minute(_) | Period(_) | Second(_) | Subsecond(_) | OffsetHour(_) | OffsetMinute(_)
            | OffsetSecond(_) | Ignore(_) | UnixTimestamp(_) | End(_),
            ..,
        ) => return Err(error::Format::InsufficientTypeInformation),
    })
}

/// Format the day into the designated output.
#[inline]
fn fmt_day(
    output: &mut (impl io::Write + ?Sized),
    date: Date,
    modifier::Day { padding }: modifier::Day,
) -> Result<usize, io::Error> {
    format_number::<2>(output, date.day(), padding)
}

/// Format the month into the designated output.
#[inline]
fn fmt_month(
    output: &mut (impl io::Write + ?Sized),
    date: Date,
    modifier::Month {
        padding,
        repr,
        case_sensitive: _, // no effect on formatting
    }: modifier::Month,
) -> Result<usize, io::Error> {
    match repr {
        modifier::MonthRepr::Numerical => {
            format_number::<2>(output, u8::from(date.month()), padding)
        }
        modifier::MonthRepr::Long => write(
            output,
            MONTH_NAMES[u8::from(date.month()).extend::<usize>() - 1],
        ),
        modifier::MonthRepr::Short => write(
            output,
            &MONTH_NAMES[u8::from(date.month()).extend::<usize>() - 1][..3],
        ),
    }
}

/// Format the ordinal into the designated output.
#[inline]
fn fmt_ordinal(
    output: &mut (impl io::Write + ?Sized),
    date: Date,
    modifier::Ordinal { padding }: modifier::Ordinal,
) -> Result<usize, io::Error> {
    format_number::<3>(output, date.ordinal(), padding)
}

/// Format the weekday into the designated output.
#[inline]
fn fmt_weekday(
    output: &mut (impl io::Write + ?Sized),
    date: Date,
    modifier::Weekday {
        repr,
        one_indexed,
        case_sensitive: _, // no effect on formatting
    }: modifier::Weekday,
) -> Result<usize, io::Error> {
    match repr {
        modifier::WeekdayRepr::Short => write(
            output,
            &WEEKDAY_NAMES[date.weekday().number_days_from_monday().extend::<usize>()][..3],
        ),
        modifier::WeekdayRepr::Long => write(
            output,
            WEEKDAY_NAMES[date.weekday().number_days_from_monday().extend::<usize>()],
        ),
        modifier::WeekdayRepr::Sunday => format_number::<1>(
            output,
            date.weekday().number_days_from_sunday() + u8::from(one_indexed),
            modifier::Padding::None,
        ),
        modifier::WeekdayRepr::Monday => format_number::<1>(
            output,
            date.weekday().number_days_from_monday() + u8::from(one_indexed),
            modifier::Padding::None,
        ),
    }
}

/// Format the week number into the designated output.
#[inline]
fn fmt_week_number(
    output: &mut (impl io::Write + ?Sized),
    date: Date,
    modifier::WeekNumber { padding, repr }: modifier::WeekNumber,
) -> Result<usize, io::Error> {
    format_number::<2>(
        output,
        match repr {
            modifier::WeekNumberRepr::Iso => date.iso_week(),
            modifier::WeekNumberRepr::Sunday => date.sunday_based_week(),
            modifier::WeekNumberRepr::Monday => date.monday_based_week(),
        },
        padding,
    )
}

/// Format the year into the designated output.
fn fmt_year(
    output: &mut (impl io::Write + ?Sized),
    date: Date,
    modifier::Year {
        padding,
        repr,
        range,
        iso_week_based,
        sign_is_mandatory,
    }: modifier::Year,
) -> Result<usize, error::Format> {
    let full_year = if iso_week_based {
        date.iso_year_week().0
    } else {
        date.year()
    };
    let value = match repr {
        modifier::YearRepr::Full => full_year,
        modifier::YearRepr::Century => full_year / 100,
        modifier::YearRepr::LastTwo => (full_year % 100).abs(),
    };
    let format_number = if cfg!(feature = "large-dates") && range == modifier::YearRange::Extended {
        match repr {
            modifier::YearRepr::Full if value.abs() >= 100_000 => format_number::<6>,
            modifier::YearRepr::Full if value.abs() >= 10_000 => format_number::<5>,
            modifier::YearRepr::Full => format_number::<4>,
            modifier::YearRepr::Century if value.abs() >= 1_000 => format_number::<4>,
            modifier::YearRepr::Century if value.abs() >= 100 => format_number::<3>,
            modifier::YearRepr::Century => format_number::<2>,
            modifier::YearRepr::LastTwo => format_number::<2>,
        }
    } else {
        match repr {
            modifier::YearRepr::Full | modifier::YearRepr::Century if full_year.abs() >= 10_000 => {
                return Err(error::ComponentRange {
                    name: "year",
                    minimum: -9999,
                    maximum: 9999,
                    value: full_year.extend(),
                    conditional_message: Some("when `range:standard` is used"),
                }
                .into());
            }
            _ => {}
        }
        match repr {
            modifier::YearRepr::Full => format_number::<4>,
            modifier::YearRepr::Century => format_number::<2>,
            modifier::YearRepr::LastTwo => format_number::<2>,
        }
    };
    let mut bytes = 0;
    if repr != modifier::YearRepr::LastTwo {
        if full_year < 0 {
            bytes += write(output, b"-")?;
        } else if sign_is_mandatory || cfg!(feature = "large-dates") && full_year >= 10_000 {
            bytes += write(output, b"+")?;
        }
    }
    bytes += format_number(output, value.unsigned_abs(), padding)?;
    Ok(bytes)
}

/// Format the hour into the designated output.
#[inline]
fn fmt_hour(
    output: &mut (impl io::Write + ?Sized),
    time: Time,
    modifier::Hour {
        padding,
        is_12_hour_clock,
    }: modifier::Hour,
) -> Result<usize, io::Error> {
    let value = match (time.hour(), is_12_hour_clock) {
        (hour, false) => hour,
        (0 | 12, true) => 12,
        (hour, true) if hour < 12 => hour,
        (hour, true) => hour - 12,
    };
    format_number::<2>(output, value, padding)
}

/// Format the minute into the designated output.
#[inline]
fn fmt_minute(
    output: &mut (impl io::Write + ?Sized),
    time: Time,
    modifier::Minute { padding }: modifier::Minute,
) -> Result<usize, io::Error> {
    format_number::<2>(output, time.minute(), padding)
}

/// Format the period into the designated output.
#[inline]
fn fmt_period(
    output: &mut (impl io::Write + ?Sized),
    time: Time,
    modifier::Period {
        is_uppercase,
        case_sensitive: _, // no effect on formatting
    }: modifier::Period,
) -> Result<usize, io::Error> {
    match (time.hour() >= 12, is_uppercase) {
        (false, false) => write(output, b"am"),
        (false, true) => write(output, b"AM"),
        (true, false) => write(output, b"pm"),
        (true, true) => write(output, b"PM"),
    }
}

/// Format the second into the designated output.
#[inline]
fn fmt_second(
    output: &mut (impl io::Write + ?Sized),
    time: Time,
    modifier::Second { padding }: modifier::Second,
) -> Result<usize, io::Error> {
    format_number::<2>(output, time.second(), padding)
}

/// Format the subsecond into the designated output.
#[inline]
fn fmt_subsecond(
    output: &mut (impl io::Write + ?Sized),
    time: Time,
    modifier::Subsecond { digits }: modifier::Subsecond,
) -> Result<usize, io::Error> {
    use modifier::SubsecondDigits::*;
    let nanos = time.nanosecond();

    if digits == Nine || (digits == OneOrMore && nanos % 10 != 0) {
        format_number_pad_zero::<9>(output, nanos)
    } else if digits == Eight || (digits == OneOrMore && (nanos / 10) % 10 != 0) {
        format_number_pad_zero::<8>(output, nanos / 10)
    } else if digits == Seven || (digits == OneOrMore && (nanos / 100) % 10 != 0) {
        format_number_pad_zero::<7>(output, nanos / 100)
    } else if digits == Six || (digits == OneOrMore && (nanos / 1_000) % 10 != 0) {
        format_number_pad_zero::<6>(output, nanos / 1_000)
    } else if digits == Five || (digits == OneOrMore && (nanos / 10_000) % 10 != 0) {
        format_number_pad_zero::<5>(output, nanos / 10_000)
    } else if digits == Four || (digits == OneOrMore && (nanos / 100_000) % 10 != 0) {
        format_number_pad_zero::<4>(output, nanos / 100_000)
    } else if digits == Three || (digits == OneOrMore && (nanos / 1_000_000) % 10 != 0) {
        format_number_pad_zero::<3>(output, nanos / 1_000_000)
    } else if digits == Two || (digits == OneOrMore && (nanos / 10_000_000) % 10 != 0) {
        format_number_pad_zero::<2>(output, nanos / 10_000_000)
    } else {
        format_number_pad_zero::<1>(output, nanos / 100_000_000)
    }
}

/// Format the offset hour into the designated output.
#[inline]
fn fmt_offset_hour(
    output: &mut (impl io::Write + ?Sized),
    offset: UtcOffset,
    modifier::OffsetHour {
        padding,
        sign_is_mandatory,
    }: modifier::OffsetHour,
) -> Result<usize, io::Error> {
    let mut bytes = 0;
    if offset.is_negative() {
        bytes += write(output, b"-")?;
    } else if sign_is_mandatory {
        bytes += write(output, b"+")?;
    }
    bytes += format_number::<2>(output, offset.whole_hours().unsigned_abs(), padding)?;
    Ok(bytes)
}

/// Format the offset minute into the designated output.
#[inline]
fn fmt_offset_minute(
    output: &mut (impl io::Write + ?Sized),
    offset: UtcOffset,
    modifier::OffsetMinute { padding }: modifier::OffsetMinute,
) -> Result<usize, io::Error> {
    format_number::<2>(output, offset.minutes_past_hour().unsigned_abs(), padding)
}

/// Format the offset second into the designated output.
#[inline]
fn fmt_offset_second(
    output: &mut (impl io::Write + ?Sized),
    offset: UtcOffset,
    modifier::OffsetSecond { padding }: modifier::OffsetSecond,
) -> Result<usize, io::Error> {
    format_number::<2>(output, offset.seconds_past_minute().unsigned_abs(), padding)
}

/// Format the Unix timestamp into the designated output.
#[inline]
fn fmt_unix_timestamp(
    output: &mut (impl io::Write + ?Sized),
    date: Date,
    time: Time,
    offset: UtcOffset,
    modifier::UnixTimestamp {
        precision,
        sign_is_mandatory,
    }: modifier::UnixTimestamp,
) -> Result<usize, io::Error> {
    let date_time = OffsetDateTime::new_in_offset(date, time, offset).to_offset(UtcOffset::UTC);

    if date_time < OffsetDateTime::UNIX_EPOCH {
        write(output, b"-")?;
    } else if sign_is_mandatory {
        write(output, b"+")?;
    }

    match precision {
        modifier::UnixTimestampPrecision::Second => {
            format_number_pad_none(output, date_time.unix_timestamp().unsigned_abs())
        }
        modifier::UnixTimestampPrecision::Millisecond => format_number_pad_none(
            output,
            (date_time.unix_timestamp_nanos() / Nanosecond::per_t::<i128>(Millisecond))
                .unsigned_abs(),
        ),
        modifier::UnixTimestampPrecision::Microsecond => format_number_pad_none(
            output,
            (date_time.unix_timestamp_nanos() / Nanosecond::per_t::<i128>(Microsecond))
                .unsigned_abs(),
        ),
        modifier::UnixTimestampPrecision::Nanosecond => {
            format_number_pad_none(output, date_time.unix_timestamp_nanos().unsigned_abs())
        }
    }
}
