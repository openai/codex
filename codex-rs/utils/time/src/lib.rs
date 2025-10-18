//! Utilities for working with UTC timestamps using only the Rust standard library.
//!
//! The helpers here intentionally avoid external time libraries so that other crates
//! in the workspace can represent timestamps without depending on `chrono`.

use std::error::Error;
use std::fmt;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const NANOS_PER_SECOND: i64 = 1_000_000_000;
const SECONDS_PER_MINUTE: i64 = 60;
const MINUTES_PER_HOUR: i64 = 60;
const HOURS_PER_DAY: i64 = 24;
const SECONDS_PER_HOUR: i64 = SECONDS_PER_MINUTE * MINUTES_PER_HOUR;
const SECONDS_PER_DAY: i64 = SECONDS_PER_HOUR * HOURS_PER_DAY;

/// Errors that can occur when parsing an RFC3339 timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    InvalidFormat,
    InvalidComponent(&'static str),
    OutOfRange,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidFormat => write!(f, "invalid RFC3339 timestamp format"),
            ParseError::InvalidComponent(component) => {
                write!(f, "invalid RFC3339 component: {component}")
            }
            ParseError::OutOfRange => write!(f, "timestamp out of range"),
        }
    }
}

impl Error for ParseError {}

#[inline]
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[inline]
fn days_in_month(year: i32, month: u32) -> Option<u32> {
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => return None,
    };
    Some(days)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Result<i64, ParseError> {
    let valid_day = days_in_month(year, month).ok_or(ParseError::InvalidComponent("month"))?;
    if day == 0 || day > valid_day {
        return Err(ParseError::InvalidComponent("day"));
    }
    let year = year - (month <= 2) as i32;
    let era = if year >= 0 {
        year as i64 / 400
    } else {
        (year as i64 - 399) / 400
    };
    let year_of_era = year as i64 - era * 400;
    let month_index = month as i64 + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month_index + 2) / 5 + day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Ok(era * 146_097 + day_of_era - 719_468)
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let mut z = days + 719_468;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    z -= era * 146_097;
    let mut doe = z;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    doe -= 365 * yoe + yoe / 4 - yoe / 100;
    let mp = (5 * doe + 2) / 153;
    let day = doe - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = era * 400 + yoe + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

#[inline]
fn div_mod_floor(n: i64, d: i64) -> (i64, i64) {
    let mut q = n / d;
    let mut r = n % d;
    if r < 0 {
        r += d;
        q -= 1;
    }
    (q, r)
}

#[inline]
fn parse_u32(src: &str) -> Option<u32> {
    if src.is_empty() || src.bytes().any(|b| !b.is_ascii_digit()) {
        return None;
    }
    src.parse().ok()
}

#[inline]
fn parse_i32(src: &str) -> Option<i32> {
    if src.len() != 4 || src.bytes().any(|b| !b.is_ascii_digit()) {
        return None;
    }
    src.parse().ok()
}

/// Parse a UTC timestamp from an RFC3339 string.
pub fn parse_rfc3339(s: &str) -> Result<SystemTime, ParseError> {
    if s.len() < 20 {
        return Err(ParseError::InvalidFormat);
    }

    let year = parse_i32(&s[0..4]).ok_or(ParseError::InvalidComponent("year"))?;
    if s.as_bytes().get(4) != Some(&b'-') {
        return Err(ParseError::InvalidFormat);
    }
    let month = parse_u32(&s[5..7]).ok_or(ParseError::InvalidComponent("month"))?;
    if s.as_bytes().get(7) != Some(&b'-') {
        return Err(ParseError::InvalidFormat);
    }
    let day = parse_u32(&s[8..10]).ok_or(ParseError::InvalidComponent("day"))?;
    if s.as_bytes().get(10) != Some(&b'T') {
        return Err(ParseError::InvalidFormat);
    }
    let hour = parse_u32(&s[11..13]).ok_or(ParseError::InvalidComponent("hour"))?;
    if s.as_bytes().get(13) != Some(&b':') {
        return Err(ParseError::InvalidFormat);
    }
    let minute = parse_u32(&s[14..16]).ok_or(ParseError::InvalidComponent("minute"))?;
    if s.as_bytes().get(16) != Some(&b':') {
        return Err(ParseError::InvalidFormat);
    }
    let second = parse_u32(&s[17..19]).ok_or(ParseError::InvalidComponent("second"))?;

    if hour > 23 {
        return Err(ParseError::InvalidComponent("hour"));
    }
    if minute > 59 {
        return Err(ParseError::InvalidComponent("minute"));
    }
    if second > 60 {
        // Allow leap seconds.
        return Err(ParseError::InvalidComponent("second"));
    }

    let mut index = 19;
    let mut nanos: i64 = 0;

    if s.as_bytes().get(index) == Some(&b'.') {
        index += 1;
        let frac_start = index;
        while let Some(b) = s.as_bytes().get(index) {
            if !b.is_ascii_digit() {
                break;
            }
            index += 1;
        }
        if frac_start == index {
            return Err(ParseError::InvalidComponent("fraction"));
        }
        let digits = &s[frac_start..index];
        if digits.len() > 9 {
            return Err(ParseError::InvalidComponent("fraction"));
        }
        let value = parse_u32(digits).ok_or(ParseError::InvalidComponent("fraction"))?;
        let scale = 9 - digits.len();
        nanos = (value as i64) * 10_i64.pow(scale as u32);
    }

    let tz = s
        .as_bytes()
        .get(index)
        .copied()
        .ok_or(ParseError::InvalidFormat)?;
    index += 1;
    let offset_seconds = match tz {
        b'Z' => 0,
        b'+' | b'-' => {
            if s.len() < index + 5 {
                return Err(ParseError::InvalidFormat);
            }
            let sign = if tz == b'+' { 1 } else { -1 };
            let hours =
                parse_u32(&s[index..index + 2]).ok_or(ParseError::InvalidComponent("offset"))?;
            if s.as_bytes().get(index + 2) != Some(&b':') {
                return Err(ParseError::InvalidFormat);
            }
            let minutes = parse_u32(&s[index + 3..index + 5])
                .ok_or(ParseError::InvalidComponent("offset"))?;
            if hours > 23 || minutes > 59 {
                return Err(ParseError::InvalidComponent("offset"));
            }
            sign * ((hours as i64 * SECONDS_PER_HOUR) + (minutes as i64 * SECONDS_PER_MINUTE))
        }
        _ => return Err(ParseError::InvalidFormat),
    };

    let days = days_from_civil(year, month, day)?;
    let mut seconds = days * SECONDS_PER_DAY
        + hour as i64 * SECONDS_PER_HOUR
        + minute as i64 * SECONDS_PER_MINUTE
        + second as i64;
    seconds -= offset_seconds;

    let total_nanos = seconds
        .checked_mul(NANOS_PER_SECOND)
        .and_then(|s| s.checked_add(nanos))
        .ok_or(ParseError::OutOfRange)?;

    system_time_from_unix_nanos(total_nanos).ok_or(ParseError::OutOfRange)
}

fn system_time_from_unix_nanos(total_nanos: i64) -> Option<SystemTime> {
    if total_nanos >= 0 {
        let secs = (total_nanos / NANOS_PER_SECOND) as u64;
        let nanos = (total_nanos % NANOS_PER_SECOND) as u32;
        Some(UNIX_EPOCH + Duration::new(secs, nanos))
    } else {
        let nanos = -total_nanos;
        let secs = (nanos / NANOS_PER_SECOND) as u64;
        let rem = (nanos % NANOS_PER_SECOND) as u32;
        let duration = Duration::new(secs, rem);
        UNIX_EPOCH.checked_sub(duration)
    }
}

fn unix_seconds_and_nanos(time: SystemTime) -> (i64, u32) {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs() as i64, duration.subsec_nanos()),
        Err(err) => {
            let duration = err.duration();
            let secs = duration.as_secs() as i64;
            let nanos = duration.subsec_nanos();
            if nanos == 0 {
                (-secs, 0)
            } else {
                (-secs - 1, 1_000_000_000 - nanos)
            }
        }
    }
}

fn split_time(time: SystemTime) -> (i32, u32, u32, u32, u32, u32, u32) {
    let (total_seconds, nanos) = unix_seconds_and_nanos(time);
    let (days, secs_of_day) = div_mod_floor(total_seconds, SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = (secs_of_day / SECONDS_PER_HOUR) as u32;
    let minute = ((secs_of_day % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE) as u32;
    let second = (secs_of_day % SECONDS_PER_MINUTE) as u32;
    (year, month, day, hour, minute, second, nanos)
}

/// UTC date-time components for display purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTimeParts {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// Extracts UTC date-time components from a `SystemTime`.
pub fn utc_datetime_parts(time: SystemTime) -> DateTimeParts {
    let (year, month, day, hour, minute, second, _) = split_time(time);
    DateTimeParts {
        year,
        month,
        day,
        hour,
        minute,
        second,
    }
}

/// Returns `true` when two instants fall on the same UTC date.
pub fn same_utc_date(a: SystemTime, b: SystemTime) -> bool {
    let (year_a, month_a, day_a, ..) = split_time(a);
    let (year_b, month_b, day_b, ..) = split_time(b);
    year_a == year_b && month_a == month_b && day_a == day_b
}

/// Format a `SystemTime` as an RFC3339 string with full nanosecond precision.
pub fn to_rfc3339(time: SystemTime) -> String {
    let (year, month, day, hour, minute, second, nanos) = split_time(time);
    if nanos == 0 {
        return format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z");
    }
    let mut frac = format!("{nanos:09}");
    while frac.ends_with('0') {
        frac.pop();
    }
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{frac}Z")
}

/// Format a `SystemTime` as an RFC3339 string with millisecond precision.
pub fn to_rfc3339_millis(time: SystemTime) -> String {
    let (year, month, day, hour, minute, second, nanos) = split_time(time);
    let millis = nanos / 1_000_000;
    if millis == 0 {
        return format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z");
    }
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Format a `SystemTime` as `YYYYMMDDTHHMMSSZ`.
pub fn to_compact_utc_string(time: SystemTime) -> String {
    let (year, month, day, hour, minute, second, _) = split_time(time);
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Formats a `SystemTime` as `Mon dd HH:MM` in UTC.
pub fn format_month_day_time_utc(time: SystemTime) -> String {
    let (year, month, day, hour, minute, _, _) = split_time(time);
    let _ = year; // keep month/day stable even for negative years
    let month_name = MONTH_NAMES
        .get((month.saturating_sub(1)) as usize)
        .unwrap_or(&"???");
    format!("{month_name} {day:>2} {hour:02}:{minute:02}")
}

/// Returns the number of whole milliseconds since the Unix epoch.
pub fn timestamp_millis(time: SystemTime) -> i128 {
    let (seconds, nanos) = unix_seconds_and_nanos(time);
    let millis = seconds as i128 * 1_000;
    let fractional = nanos as i128 / 1_000_000;
    millis + fractional
}

/// Returns the duration between two points in time, clamping at zero.
pub fn saturating_duration_since(later: SystemTime, earlier: SystemTime) -> Duration {
    later
        .duration_since(earlier)
        .unwrap_or_else(|_| Duration::from_secs(0))
}

/// Construct a timestamp from a Unix timestamp expressed as seconds (with fractional nanoseconds).
pub fn from_unix_seconds_f64(ts: f64) -> SystemTime {
    if !ts.is_finite() {
        return UNIX_EPOCH;
    }
    let secs = ts.trunc() as i64;
    let nanos = ((ts - secs as f64) * NANOS_PER_SECOND as f64).round() as i64;
    let total = secs
        .checked_mul(NANOS_PER_SECOND)
        .and_then(|s| s.checked_add(nanos))
        .unwrap_or(0);
    system_time_from_unix_nanos(total).unwrap_or(UNIX_EPOCH)
}

#[cfg(feature = "serde")]
pub mod serde {
    use super::*;
    
    
    
    use ::serde::de::Error as _;

    pub mod rfc3339 {
        use super::*;
        use ::serde::Deserialize;
        use ::serde::Deserializer;
        use ::serde::Serializer;

        pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&to_rfc3339(*time))
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s = String::deserialize(deserializer)?;
            parse_rfc3339(&s).map_err(D::Error::custom)
        }
    }

    pub mod option_rfc3339 {
        use super::*;
        use ::serde::Deserialize;
        use ::serde::Deserializer;
        use ::serde::Serializer;

        pub fn serialize<S>(time: &Option<SystemTime>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match time {
                Some(t) => serializer.serialize_some(&to_rfc3339(*t)),
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<SystemTime>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let opt = Option::<String>::deserialize(deserializer)?;
            opt.map(|s| parse_rfc3339(&s).map_err(D::Error::custom))
                .transpose()
        }
    }
}
