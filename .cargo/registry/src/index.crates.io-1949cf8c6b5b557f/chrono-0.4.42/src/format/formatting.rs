// This is a part of Chrono.
// See README.md and LICENSE.txt for details.

//! Date and time formatting routines.

#[cfg(all(feature = "alloc", not(feature = "std"), not(test)))]
use alloc::string::{String, ToString};
#[cfg(feature = "alloc")]
use core::borrow::Borrow;
#[cfg(feature = "alloc")]
use core::fmt::Display;
use core::fmt::{self, Write};

#[cfg(feature = "alloc")]
use crate::offset::Offset;
#[cfg(any(feature = "alloc", feature = "serde"))]
use crate::{Datelike, FixedOffset, NaiveDateTime, Timelike};
#[cfg(feature = "alloc")]
use crate::{NaiveDate, NaiveTime, Weekday};

#[cfg(feature = "alloc")]
use super::locales;
#[cfg(any(feature = "alloc", feature = "serde"))]
use super::{Colons, OffsetFormat, OffsetPrecision, Pad};
#[cfg(feature = "alloc")]
use super::{Fixed, InternalFixed, InternalInternal, Item, Numeric};
#[cfg(feature = "alloc")]
use locales::*;

/// A *temporary* object which can be used as an argument to `format!` or others.
/// This is normally constructed via `format` methods of each date and time type.
#[cfg(feature = "alloc")]
#[derive(Debug)]
pub struct DelayedFormat<I> {
    /// The date view, if any.
    date: Option<NaiveDate>,
    /// The time view, if any.
    time: Option<NaiveTime>,
    /// The name and local-to-UTC difference for the offset (timezone), if any.
    off: Option<(String, FixedOffset)>,
    /// An iterator returning formatting items.
    items: I,
    /// Locale used for text.
    /// ZST if the `unstable-locales` feature is not enabled.
    locale: Locale,
}

#[cfg(feature = "alloc")]
impl<'a, I: Iterator<Item = B> + Clone, B: Borrow<Item<'a>>> DelayedFormat<I> {
    /// Makes a new `DelayedFormat` value out of local date and time.
    #[must_use]
    pub fn new(date: Option<NaiveDate>, time: Option<NaiveTime>, items: I) -> DelayedFormat<I> {
        DelayedFormat { date, time, off: None, items, locale: default_locale() }
    }

    /// Makes a new `DelayedFormat` value out of local date and time and UTC offset.
    #[must_use]
    pub fn new_with_offset<Off>(
        date: Option<NaiveDate>,
        time: Option<NaiveTime>,
        offset: &Off,
        items: I,
    ) -> DelayedFormat<I>
    where
        Off: Offset + Display,
    {
        let name_and_diff = (offset.to_string(), offset.fix());
        DelayedFormat { date, time, off: Some(name_and_diff), items, locale: default_locale() }
    }

    /// Makes a new `DelayedFormat` value out of local date and time and locale.
    #[cfg(feature = "unstable-locales")]
    #[must_use]
    pub fn new_with_locale(
        date: Option<NaiveDate>,
        time: Option<NaiveTime>,
        items: I,
        locale: Locale,
    ) -> DelayedFormat<I> {
        DelayedFormat { date, time, off: None, items, locale }
    }

    /// Makes a new `DelayedFormat` value out of local date and time, UTC offset and locale.
    #[cfg(feature = "unstable-locales")]
    #[must_use]
    pub fn new_with_offset_and_locale<Off>(
        date: Option<NaiveDate>,
        time: Option<NaiveTime>,
        offset: &Off,
        items: I,
        locale: Locale,
    ) -> DelayedFormat<I>
    where
        Off: Offset + Display,
    {
        let name_and_diff = (offset.to_string(), offset.fix());
        DelayedFormat { date, time, off: Some(name_and_diff), items, locale }
    }

    /// Formats `DelayedFormat` into a `core::fmt::Write` instance.
    /// # Errors
    /// This function returns a `core::fmt::Error` if formatting into the `core::fmt::Write` instance fails.
    ///
    /// # Example
    /// ### Writing to a String
    /// ```
    /// let dt = chrono::DateTime::from_timestamp(1643723400, 123456789).unwrap();
    /// let df = dt.format("%Y-%m-%d %H:%M:%S%.9f");
    /// let mut buffer = String::new();
    /// let _ = df.write_to(&mut buffer);
    /// ```
    pub fn write_to(&self, w: &mut (impl Write + ?Sized)) -> fmt::Result {
        for item in self.items.clone() {
            match *item.borrow() {
                Item::Literal(s) | Item::Space(s) => w.write_str(s),
                #[cfg(feature = "alloc")]
                Item::OwnedLiteral(ref s) | Item::OwnedSpace(ref s) => w.write_str(s),
                Item::Numeric(ref spec, pad) => self.format_numeric(w, spec, pad),
                Item::Fixed(ref spec) => self.format_fixed(w, spec),
                Item::Error => Err(fmt::Error),
            }?;
        }
        Ok(())
    }

    #[cfg(feature = "alloc")]
    fn format_numeric(
        &self,
        w: &mut (impl Write + ?Sized),
        spec: &Numeric,
        pad: Pad,
    ) -> fmt::Result {
        use self::Numeric::*;

        fn write_one(w: &mut (impl Write + ?Sized), v: u8) -> fmt::Result {
            w.write_char((b'0' + v) as char)
        }

        fn write_two(w: &mut (impl Write + ?Sized), v: u8, pad: Pad) -> fmt::Result {
            let ones = b'0' + v % 10;
            match (v / 10, pad) {
                (0, Pad::None) => {}
                (0, Pad::Space) => w.write_char(' ')?,
                (tens, _) => w.write_char((b'0' + tens) as char)?,
            }
            w.write_char(ones as char)
        }

        #[inline]
        fn write_year(w: &mut (impl Write + ?Sized), year: i32, pad: Pad) -> fmt::Result {
            if (1000..=9999).contains(&year) {
                // fast path
                write_hundreds(w, (year / 100) as u8)?;
                write_hundreds(w, (year % 100) as u8)
            } else {
                write_n(w, 4, year as i64, pad, !(0..10_000).contains(&year))
            }
        }

        fn write_n(
            w: &mut (impl Write + ?Sized),
            n: usize,
            v: i64,
            pad: Pad,
            always_sign: bool,
        ) -> fmt::Result {
            if always_sign {
                match pad {
                    Pad::None => write!(w, "{v:+}"),
                    Pad::Zero => write!(w, "{:+01$}", v, n + 1),
                    Pad::Space => write!(w, "{:+1$}", v, n + 1),
                }
            } else {
                match pad {
                    Pad::None => write!(w, "{v}"),
                    Pad::Zero => write!(w, "{v:0n$}"),
                    Pad::Space => write!(w, "{v:n$}"),
                }
            }
        }

        match (spec, self.date, self.time) {
            (Year, Some(d), _) => write_year(w, d.year(), pad),
            (YearDiv100, Some(d), _) => write_two(w, d.year().div_euclid(100) as u8, pad),
            (YearMod100, Some(d), _) => write_two(w, d.year().rem_euclid(100) as u8, pad),
            (IsoYear, Some(d), _) => write_year(w, d.iso_week().year(), pad),
            (IsoYearDiv100, Some(d), _) => {
                write_two(w, d.iso_week().year().div_euclid(100) as u8, pad)
            }
            (IsoYearMod100, Some(d), _) => {
                write_two(w, d.iso_week().year().rem_euclid(100) as u8, pad)
            }
            (Quarter, Some(d), _) => write_one(w, d.quarter() as u8),
            (Month, Some(d), _) => write_two(w, d.month() as u8, pad),
            (Day, Some(d), _) => write_two(w, d.day() as u8, pad),
            (WeekFromSun, Some(d), _) => write_two(w, d.weeks_from(Weekday::Sun) as u8, pad),
            (WeekFromMon, Some(d), _) => write_two(w, d.weeks_from(Weekday::Mon) as u8, pad),
            (IsoWeek, Some(d), _) => write_two(w, d.iso_week().week() as u8, pad),
            (NumDaysFromSun, Some(d), _) => write_one(w, d.weekday().num_days_from_sunday() as u8),
            (WeekdayFromMon, Some(d), _) => write_one(w, d.weekday().number_from_monday() as u8),
            (Ordinal, Some(d), _) => write_n(w, 3, d.ordinal() as i64, pad, false),
            (Hour, _, Some(t)) => write_two(w, t.hour() as u8, pad),
            (Hour12, _, Some(t)) => write_two(w, t.hour12().1 as u8, pad),
            (Minute, _, Some(t)) => write_two(w, t.minute() as u8, pad),
            (Second, _, Some(t)) => {
                write_two(w, (t.second() + t.nanosecond() / 1_000_000_000) as u8, pad)
            }
            (Nanosecond, _, Some(t)) => {
                write_n(w, 9, (t.nanosecond() % 1_000_000_000) as i64, pad, false)
            }
            (Timestamp, Some(d), Some(t)) => {
                let offset = self.off.as_ref().map(|(_, o)| i64::from(o.local_minus_utc()));
                let timestamp = d.and_time(t).and_utc().timestamp() - offset.unwrap_or(0);
                write_n(w, 9, timestamp, pad, false)
            }
            (Internal(_), _, _) => Ok(()), // for future expansion
            _ => Err(fmt::Error),          // insufficient arguments for given format
        }
    }

    #[cfg(feature = "alloc")]
    fn format_fixed(&self, w: &mut (impl Write + ?Sized), spec: &Fixed) -> fmt::Result {
        use Fixed::*;
        use InternalInternal::*;

        match (spec, self.date, self.time, self.off.as_ref()) {
            (ShortMonthName, Some(d), _, _) => {
                w.write_str(short_months(self.locale)[d.month0() as usize])
            }
            (LongMonthName, Some(d), _, _) => {
                w.write_str(long_months(self.locale)[d.month0() as usize])
            }
            (ShortWeekdayName, Some(d), _, _) => w.write_str(
                short_weekdays(self.locale)[d.weekday().num_days_from_sunday() as usize],
            ),
            (LongWeekdayName, Some(d), _, _) => {
                w.write_str(long_weekdays(self.locale)[d.weekday().num_days_from_sunday() as usize])
            }
            (LowerAmPm, _, Some(t), _) => {
                let ampm = if t.hour12().0 { am_pm(self.locale)[1] } else { am_pm(self.locale)[0] };
                for c in ampm.chars().flat_map(|c| c.to_lowercase()) {
                    w.write_char(c)?
                }
                Ok(())
            }
            (UpperAmPm, _, Some(t), _) => {
                let ampm = if t.hour12().0 { am_pm(self.locale)[1] } else { am_pm(self.locale)[0] };
                w.write_str(ampm)
            }
            (Nanosecond, _, Some(t), _) => {
                let nano = t.nanosecond() % 1_000_000_000;
                if nano == 0 {
                    Ok(())
                } else {
                    w.write_str(decimal_point(self.locale))?;
                    if nano % 1_000_000 == 0 {
                        write!(w, "{:03}", nano / 1_000_000)
                    } else if nano % 1_000 == 0 {
                        write!(w, "{:06}", nano / 1_000)
                    } else {
                        write!(w, "{nano:09}")
                    }
                }
            }
            (Nanosecond3, _, Some(t), _) => {
                w.write_str(decimal_point(self.locale))?;
                write!(w, "{:03}", t.nanosecond() / 1_000_000 % 1000)
            }
            (Nanosecond6, _, Some(t), _) => {
                w.write_str(decimal_point(self.locale))?;
                write!(w, "{:06}", t.nanosecond() / 1_000 % 1_000_000)
            }
            (Nanosecond9, _, Some(t), _) => {
                w.write_str(decimal_point(self.locale))?;
                write!(w, "{:09}", t.nanosecond() % 1_000_000_000)
            }
            (Internal(InternalFixed { val: Nanosecond3NoDot }), _, Some(t), _) => {
                write!(w, "{:03}", t.nanosecond() / 1_000_000 % 1_000)
            }
            (Internal(InternalFixed { val: Nanosecond6NoDot }), _, Some(t), _) => {
                write!(w, "{:06}", t.nanosecond() / 1_000 % 1_000_000)
            }
            (Internal(InternalFixed { val: Nanosecond9NoDot }), _, Some(t), _) => {
                write!(w, "{:09}", t.nanosecond() % 1_000_000_000)
            }
            (TimezoneName, _, _, Some((tz_name, _))) => write!(w, "{tz_name}"),
            (TimezoneOffset | TimezoneOffsetZ, _, _, Some((_, off))) => {
                let offset_format = OffsetFormat {
                    precision: OffsetPrecision::Minutes,
                    colons: Colons::Maybe,
                    allow_zulu: *spec == TimezoneOffsetZ,
                    padding: Pad::Zero,
                };
                offset_format.format(w, *off)
            }
            (TimezoneOffsetColon | TimezoneOffsetColonZ, _, _, Some((_, off))) => {
                let offset_format = OffsetFormat {
                    precision: OffsetPrecision::Minutes,
                    colons: Colons::Colon,
                    allow_zulu: *spec == TimezoneOffsetColonZ,
                    padding: Pad::Zero,
                };
                offset_format.format(w, *off)
            }
            (TimezoneOffsetDoubleColon, _, _, Some((_, off))) => {
                let offset_format = OffsetFormat {
                    precision: OffsetPrecision::Seconds,
                    colons: Colons::Colon,
                    allow_zulu: false,
                    padding: Pad::Zero,
                };
                offset_format.format(w, *off)
            }
            (TimezoneOffsetTripleColon, _, _, Some((_, off))) => {
                let offset_format = OffsetFormat {
                    precision: OffsetPrecision::Hours,
                    colons: Colons::None,
                    allow_zulu: false,
                    padding: Pad::Zero,
                };
                offset_format.format(w, *off)
            }
            (RFC2822, Some(d), Some(t), Some((_, off))) => {
                write_rfc2822(w, crate::NaiveDateTime::new(d, t), *off)
            }
            (RFC3339, Some(d), Some(t), Some((_, off))) => write_rfc3339(
                w,
                crate::NaiveDateTime::new(d, t),
                *off,
                SecondsFormat::AutoSi,
                false,
            ),
            _ => Err(fmt::Error), // insufficient arguments for given format
        }
    }
}

#[cfg(feature = "alloc")]
impl<'a, I: Iterator<Item = B> + Clone, B: Borrow<Item<'a>>> Display for DelayedFormat<I> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = String::new();
        self.write_to(&mut result)?;
        f.pad(&result)
    }
}

/// Tries to format given arguments with given formatting items.
/// Internally used by `DelayedFormat`.
#[cfg(feature = "alloc")]
#[deprecated(since = "0.4.32", note = "Use DelayedFormat::fmt or DelayedFormat::write_to instead")]
pub fn format<'a, I, B>(
    w: &mut fmt::Formatter,
    date: Option<&NaiveDate>,
    time: Option<&NaiveTime>,
    off: Option<&(String, FixedOffset)>,
    items: I,
) -> fmt::Result
where
    I: Iterator<Item = B> + Clone,
    B: Borrow<Item<'a>>,
{
    DelayedFormat {
        date: date.copied(),
        time: time.copied(),
        off: off.cloned(),
        items,
        locale: default_locale(),
    }
    .fmt(w)
}

/// Formats single formatting item
#[cfg(feature = "alloc")]
#[deprecated(since = "0.4.32", note = "Use DelayedFormat::fmt or DelayedFormat::write_to instead")]
pub fn format_item(
    w: &mut fmt::Formatter,
    date: Option<&NaiveDate>,
    time: Option<&NaiveTime>,
    off: Option<&(String, FixedOffset)>,
    item: &Item<'_>,
) -> fmt::Result {
    DelayedFormat {
        date: date.copied(),
        time: time.copied(),
        off: off.cloned(),
        items: [item].into_iter(),
        locale: default_locale(),
    }
    .fmt(w)
}

#[cfg(any(feature = "alloc", feature = "serde"))]
impl OffsetFormat {
    /// Writes an offset from UTC with the format defined by `self`.
    fn format(&self, w: &mut (impl Write + ?Sized), off: FixedOffset) -> fmt::Result {
        let off = off.local_minus_utc();
        if self.allow_zulu && off == 0 {
            w.write_char('Z')?;
            return Ok(());
        }
        let (sign, off) = if off < 0 { ('-', -off) } else { ('+', off) };

        let hours;
        let mut mins = 0;
        let mut secs = 0;
        let precision = match self.precision {
            OffsetPrecision::Hours => {
                // Minutes and seconds are simply truncated
                hours = (off / 3600) as u8;
                OffsetPrecision::Hours
            }
            OffsetPrecision::Minutes | OffsetPrecision::OptionalMinutes => {
                // Round seconds to the nearest minute.
                let minutes = (off + 30) / 60;
                mins = (minutes % 60) as u8;
                hours = (minutes / 60) as u8;
                if self.precision == OffsetPrecision::OptionalMinutes && mins == 0 {
                    OffsetPrecision::Hours
                } else {
                    OffsetPrecision::Minutes
                }
            }
            OffsetPrecision::Seconds
            | OffsetPrecision::OptionalSeconds
            | OffsetPrecision::OptionalMinutesAndSeconds => {
                let minutes = off / 60;
                secs = (off % 60) as u8;
                mins = (minutes % 60) as u8;
                hours = (minutes / 60) as u8;
                if self.precision != OffsetPrecision::Seconds && secs == 0 {
                    if self.precision == OffsetPrecision::OptionalMinutesAndSeconds && mins == 0 {
                        OffsetPrecision::Hours
                    } else {
                        OffsetPrecision::Minutes
                    }
                } else {
                    OffsetPrecision::Seconds
                }
            }
        };
        let colons = self.colons == Colons::Colon;

        if hours < 10 {
            if self.padding == Pad::Space {
                w.write_char(' ')?;
            }
            w.write_char(sign)?;
            if self.padding == Pad::Zero {
                w.write_char('0')?;
            }
            w.write_char((b'0' + hours) as char)?;
        } else {
            w.write_char(sign)?;
            write_hundreds(w, hours)?;
        }
        if let OffsetPrecision::Minutes | OffsetPrecision::Seconds = precision {
            if colons {
                w.write_char(':')?;
            }
            write_hundreds(w, mins)?;
        }
        if let OffsetPrecision::Seconds = precision {
            if colons {
                w.write_char(':')?;
            }
            write_hundreds(w, secs)?;
        }
        Ok(())
    }
}

/// Specific formatting options for seconds. This may be extended in the
/// future, so exhaustive matching in external code is not recommended.
///
/// See the `TimeZone::to_rfc3339_opts` function for usage.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[allow(clippy::manual_non_exhaustive)]
pub enum SecondsFormat {
    /// Format whole seconds only, with no decimal point nor subseconds.
    Secs,

    /// Use fixed 3 subsecond digits. This corresponds to [Fixed::Nanosecond3].
    Millis,

    /// Use fixed 6 subsecond digits. This corresponds to [Fixed::Nanosecond6].
    Micros,

    /// Use fixed 9 subsecond digits. This corresponds to [Fixed::Nanosecond9].
    Nanos,

    /// Automatically select one of `Secs`, `Millis`, `Micros`, or `Nanos` to display all available
    /// non-zero sub-second digits.  This corresponds to [Fixed::Nanosecond].
    AutoSi,

    // Do not match against this.
    #[doc(hidden)]
    __NonExhaustive,
}

/// Writes the date, time and offset to the string. same as `%Y-%m-%dT%H:%M:%S%.f%:z`
#[inline]
#[cfg(any(feature = "alloc", feature = "serde"))]
pub(crate) fn write_rfc3339(
    w: &mut (impl Write + ?Sized),
    dt: NaiveDateTime,
    off: FixedOffset,
    secform: SecondsFormat,
    use_z: bool,
) -> fmt::Result {
    let year = dt.date().year();
    if (0..=9999).contains(&year) {
        write_hundreds(w, (year / 100) as u8)?;
        write_hundreds(w, (year % 100) as u8)?;
    } else {
        // ISO 8601 requires the explicit sign for out-of-range years
        write!(w, "{year:+05}")?;
    }
    w.write_char('-')?;
    write_hundreds(w, dt.date().month() as u8)?;
    w.write_char('-')?;
    write_hundreds(w, dt.date().day() as u8)?;

    w.write_char('T')?;

    let (hour, min, mut sec) = dt.time().hms();
    let mut nano = dt.nanosecond();
    if nano >= 1_000_000_000 {
        sec += 1;
        nano -= 1_000_000_000;
    }
    write_hundreds(w, hour as u8)?;
    w.write_char(':')?;
    write_hundreds(w, min as u8)?;
    w.write_char(':')?;
    let sec = sec;
    write_hundreds(w, sec as u8)?;

    match secform {
        SecondsFormat::Secs => {}
        SecondsFormat::Millis => write!(w, ".{:03}", nano / 1_000_000)?,
        SecondsFormat::Micros => write!(w, ".{:06}", nano / 1000)?,
        SecondsFormat::Nanos => write!(w, ".{nano:09}")?,
        SecondsFormat::AutoSi => {
            if nano == 0 {
            } else if nano % 1_000_000 == 0 {
                write!(w, ".{:03}", nano / 1_000_000)?
            } else if nano % 1_000 == 0 {
                write!(w, ".{:06}", nano / 1_000)?
            } else {
                write!(w, ".{nano:09}")?
            }
        }
        SecondsFormat::__NonExhaustive => unreachable!(),
    };

    OffsetFormat {
        precision: OffsetPrecision::Minutes,
        colons: Colons::Colon,
        allow_zulu: use_z,
        padding: Pad::Zero,
    }
    .format(w, off)
}

#[cfg(feature = "alloc")]
/// write datetimes like `Tue, 1 Jul 2003 10:52:37 +0200`, same as `%a, %d %b %Y %H:%M:%S %z`
pub(crate) fn write_rfc2822(
    w: &mut (impl Write + ?Sized),
    dt: NaiveDateTime,
    off: FixedOffset,
) -> fmt::Result {
    let year = dt.year();
    // RFC2822 is only defined on years 0 through 9999
    if !(0..=9999).contains(&year) {
        return Err(fmt::Error);
    }

    let english = default_locale();

    w.write_str(short_weekdays(english)[dt.weekday().num_days_from_sunday() as usize])?;
    w.write_str(", ")?;
    let day = dt.day();
    if day < 10 {
        w.write_char((b'0' + day as u8) as char)?;
    } else {
        write_hundreds(w, day as u8)?;
    }
    w.write_char(' ')?;
    w.write_str(short_months(english)[dt.month0() as usize])?;
    w.write_char(' ')?;
    write_hundreds(w, (year / 100) as u8)?;
    write_hundreds(w, (year % 100) as u8)?;
    w.write_char(' ')?;

    let (hour, min, sec) = dt.time().hms();
    write_hundreds(w, hour as u8)?;
    w.write_char(':')?;
    write_hundreds(w, min as u8)?;
    w.write_char(':')?;
    let sec = sec + dt.nanosecond() / 1_000_000_000;
    write_hundreds(w, sec as u8)?;
    w.write_char(' ')?;
    OffsetFormat {
        precision: OffsetPrecision::Minutes,
        colons: Colons::None,
        allow_zulu: false,
        padding: Pad::Zero,
    }
    .format(w, off)
}

/// Equivalent to `{:02}` formatting for n < 100.
pub(crate) fn write_hundreds(w: &mut (impl Write + ?Sized), n: u8) -> fmt::Result {
    if n >= 100 {
        return Err(fmt::Error);
    }

    let tens = b'0' + n / 10;
    let ones = b'0' + n % 10;
    w.write_char(tens as char)?;
    w.write_char(ones as char)
}

#[cfg(test)]
#[cfg(feature = "alloc")]
mod tests {
    use super::{Colons, OffsetFormat, OffsetPrecision, Pad};
    use crate::FixedOffset;
    #[cfg(feature = "alloc")]
    use crate::{NaiveDate, NaiveTime, TimeZone, Timelike, Utc};

    #[cfg(feature = "alloc")]
    #[test]
    fn test_delayed_write_to() {
        let dt = crate::DateTime::from_timestamp(1643723400, 123456789).unwrap();
        let df = dt.format("%Y-%m-%d %H:%M:%S%.9f");

        let mut dt_str = String::new();

        df.write_to(&mut dt_str).unwrap();
        assert_eq!(dt_str, "2022-02-01 13:50:00.123456789");
    }

    #[cfg(all(feature = "std", feature = "unstable-locales", feature = "alloc"))]
    #[test]
    fn test_with_locale_delayed_write_to() {
        use crate::DateTime;
        use crate::format::locales::Locale;

        let dt = DateTime::from_timestamp(1643723400, 123456789).unwrap();
        let df = dt.format_localized("%A, %B %d, %Y", Locale::ja_JP);

        let mut dt_str = String::new();

        df.write_to(&mut dt_str).unwrap();

        assert_eq!(dt_str, "火曜日, 2月 01, 2022");
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_date_format() {
        let d = NaiveDate::from_ymd_opt(2012, 3, 4).unwrap();
        assert_eq!(d.format("%Y,%C,%y,%G,%g").to_string(), "2012,20,12,2012,12");
        assert_eq!(d.format("%m,%b,%h,%B").to_string(), "03,Mar,Mar,March");
        assert_eq!(d.format("%q").to_string(), "1");
        assert_eq!(d.format("%d,%e").to_string(), "04, 4");
        assert_eq!(d.format("%U,%W,%V").to_string(), "10,09,09");
        assert_eq!(d.format("%a,%A,%w,%u").to_string(), "Sun,Sunday,0,7");
        assert_eq!(d.format("%j").to_string(), "064"); // since 2012 is a leap year
        assert_eq!(d.format("%D,%x").to_string(), "03/04/12,03/04/12");
        assert_eq!(d.format("%F").to_string(), "2012-03-04");
        assert_eq!(d.format("%v").to_string(), " 4-Mar-2012");
        assert_eq!(d.format("%t%n%%%n%t").to_string(), "\t\n%\n\t");

        // non-four-digit years
        assert_eq!(
            NaiveDate::from_ymd_opt(12345, 1, 1).unwrap().format("%Y").to_string(),
            "+12345"
        );
        assert_eq!(NaiveDate::from_ymd_opt(1234, 1, 1).unwrap().format("%Y").to_string(), "1234");
        assert_eq!(NaiveDate::from_ymd_opt(123, 1, 1).unwrap().format("%Y").to_string(), "0123");
        assert_eq!(NaiveDate::from_ymd_opt(12, 1, 1).unwrap().format("%Y").to_string(), "0012");
        assert_eq!(NaiveDate::from_ymd_opt(1, 1, 1).unwrap().format("%Y").to_string(), "0001");
        assert_eq!(NaiveDate::from_ymd_opt(0, 1, 1).unwrap().format("%Y").to_string(), "0000");
        assert_eq!(NaiveDate::from_ymd_opt(-1, 1, 1).unwrap().format("%Y").to_string(), "-0001");
        assert_eq!(NaiveDate::from_ymd_opt(-12, 1, 1).unwrap().format("%Y").to_string(), "-0012");
        assert_eq!(NaiveDate::from_ymd_opt(-123, 1, 1).unwrap().format("%Y").to_string(), "-0123");
        assert_eq!(NaiveDate::from_ymd_opt(-1234, 1, 1).unwrap().format("%Y").to_string(), "-1234");
        assert_eq!(
            NaiveDate::from_ymd_opt(-12345, 1, 1).unwrap().format("%Y").to_string(),
            "-12345"
        );

        // corner cases
        assert_eq!(
            NaiveDate::from_ymd_opt(2007, 12, 31).unwrap().format("%G,%g,%U,%W,%V").to_string(),
            "2008,08,52,53,01"
        );
        assert_eq!(
            NaiveDate::from_ymd_opt(2010, 1, 3).unwrap().format("%G,%g,%U,%W,%V").to_string(),
            "2009,09,01,00,53"
        );
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_time_format() {
        let t = NaiveTime::from_hms_nano_opt(3, 5, 7, 98765432).unwrap();
        assert_eq!(t.format("%H,%k,%I,%l,%P,%p").to_string(), "03, 3,03, 3,am,AM");
        assert_eq!(t.format("%M").to_string(), "05");
        assert_eq!(t.format("%S,%f,%.f").to_string(), "07,098765432,.098765432");
        assert_eq!(t.format("%.3f,%.6f,%.9f").to_string(), ".098,.098765,.098765432");
        assert_eq!(t.format("%R").to_string(), "03:05");
        assert_eq!(t.format("%T,%X").to_string(), "03:05:07,03:05:07");
        assert_eq!(t.format("%r").to_string(), "03:05:07 AM");
        assert_eq!(t.format("%t%n%%%n%t").to_string(), "\t\n%\n\t");

        let t = NaiveTime::from_hms_micro_opt(3, 5, 7, 432100).unwrap();
        assert_eq!(t.format("%S,%f,%.f").to_string(), "07,432100000,.432100");
        assert_eq!(t.format("%.3f,%.6f,%.9f").to_string(), ".432,.432100,.432100000");

        let t = NaiveTime::from_hms_milli_opt(3, 5, 7, 210).unwrap();
        assert_eq!(t.format("%S,%f,%.f").to_string(), "07,210000000,.210");
        assert_eq!(t.format("%.3f,%.6f,%.9f").to_string(), ".210,.210000,.210000000");

        let t = NaiveTime::from_hms_opt(3, 5, 7).unwrap();
        assert_eq!(t.format("%S,%f,%.f").to_string(), "07,000000000,");
        assert_eq!(t.format("%.3f,%.6f,%.9f").to_string(), ".000,.000000,.000000000");

        // corner cases
        assert_eq!(
            NaiveTime::from_hms_opt(13, 57, 9).unwrap().format("%r").to_string(),
            "01:57:09 PM"
        );
        assert_eq!(
            NaiveTime::from_hms_milli_opt(23, 59, 59, 1_000).unwrap().format("%X").to_string(),
            "23:59:60"
        );
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_datetime_format() {
        let dt =
            NaiveDate::from_ymd_opt(2010, 9, 8).unwrap().and_hms_milli_opt(7, 6, 54, 321).unwrap();
        assert_eq!(dt.format("%c").to_string(), "Wed Sep  8 07:06:54 2010");
        assert_eq!(dt.format("%s").to_string(), "1283929614");
        assert_eq!(dt.format("%t%n%%%n%t").to_string(), "\t\n%\n\t");

        // a horror of leap second: coming near to you.
        let dt = NaiveDate::from_ymd_opt(2012, 6, 30)
            .unwrap()
            .and_hms_milli_opt(23, 59, 59, 1_000)
            .unwrap();
        assert_eq!(dt.format("%c").to_string(), "Sat Jun 30 23:59:60 2012");
        assert_eq!(dt.format("%s").to_string(), "1341100799"); // not 1341100800, it's intentional.
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_datetime_format_alignment() {
        let datetime = Utc
            .with_ymd_and_hms(2007, 1, 2, 12, 34, 56)
            .unwrap()
            .with_nanosecond(123456789)
            .unwrap();

        // Item::Literal, odd number of padding bytes.
        let percent = datetime.format("%%");
        assert_eq!("   %", format!("{percent:>4}"));
        assert_eq!("%   ", format!("{percent:<4}"));
        assert_eq!(" %  ", format!("{percent:^4}"));

        // Item::Numeric, custom non-ASCII padding character
        let year = datetime.format("%Y");
        assert_eq!("——2007", format!("{year:—>6}"));
        assert_eq!("2007——", format!("{year:—<6}"));
        assert_eq!("—2007—", format!("{year:—^6}"));

        // Item::Fixed
        let tz = datetime.format("%Z");
        assert_eq!("  UTC", format!("{tz:>5}"));
        assert_eq!("UTC  ", format!("{tz:<5}"));
        assert_eq!(" UTC ", format!("{tz:^5}"));

        // [Item::Numeric, Item::Space, Item::Literal, Item::Space, Item::Numeric]
        let ymd = datetime.format("%Y %B %d");
        assert_eq!("  2007 January 02", format!("{ymd:>17}"));
        assert_eq!("2007 January 02  ", format!("{ymd:<17}"));
        assert_eq!(" 2007 January 02 ", format!("{ymd:^17}"));

        // Truncated
        let time = datetime.format("%T%.6f");
        assert_eq!("12:34:56.1234", format!("{time:.13}"));
    }

    #[test]
    fn test_offset_formatting() {
        fn check_all(precision: OffsetPrecision, expected: [[&str; 7]; 12]) {
            fn check(
                precision: OffsetPrecision,
                colons: Colons,
                padding: Pad,
                allow_zulu: bool,
                offsets: [FixedOffset; 7],
                expected: [&str; 7],
            ) {
                let offset_format = OffsetFormat { precision, colons, allow_zulu, padding };
                for (offset, expected) in offsets.iter().zip(expected.iter()) {
                    let mut output = String::new();
                    offset_format.format(&mut output, *offset).unwrap();
                    assert_eq!(&output, expected);
                }
            }
            // +03:45, -03:30, +11:00, -11:00:22, +02:34:26, -12:34:30, +00:00
            let offsets = [
                FixedOffset::east_opt(13_500).unwrap(),
                FixedOffset::east_opt(-12_600).unwrap(),
                FixedOffset::east_opt(39_600).unwrap(),
                FixedOffset::east_opt(-39_622).unwrap(),
                FixedOffset::east_opt(9266).unwrap(),
                FixedOffset::east_opt(-45270).unwrap(),
                FixedOffset::east_opt(0).unwrap(),
            ];
            check(precision, Colons::Colon, Pad::Zero, false, offsets, expected[0]);
            check(precision, Colons::Colon, Pad::Zero, true, offsets, expected[1]);
            check(precision, Colons::Colon, Pad::Space, false, offsets, expected[2]);
            check(precision, Colons::Colon, Pad::Space, true, offsets, expected[3]);
            check(precision, Colons::Colon, Pad::None, false, offsets, expected[4]);
            check(precision, Colons::Colon, Pad::None, true, offsets, expected[5]);
            check(precision, Colons::None, Pad::Zero, false, offsets, expected[6]);
            check(precision, Colons::None, Pad::Zero, true, offsets, expected[7]);
            check(precision, Colons::None, Pad::Space, false, offsets, expected[8]);
            check(precision, Colons::None, Pad::Space, true, offsets, expected[9]);
            check(precision, Colons::None, Pad::None, false, offsets, expected[10]);
            check(precision, Colons::None, Pad::None, true, offsets, expected[11]);
            // `Colons::Maybe` should format the same as `Colons::None`
            check(precision, Colons::Maybe, Pad::Zero, false, offsets, expected[6]);
            check(precision, Colons::Maybe, Pad::Zero, true, offsets, expected[7]);
            check(precision, Colons::Maybe, Pad::Space, false, offsets, expected[8]);
            check(precision, Colons::Maybe, Pad::Space, true, offsets, expected[9]);
            check(precision, Colons::Maybe, Pad::None, false, offsets, expected[10]);
            check(precision, Colons::Maybe, Pad::None, true, offsets, expected[11]);
        }
        check_all(
            OffsetPrecision::Hours,
            [
                ["+03", "-03", "+11", "-11", "+02", "-12", "+00"],
                ["+03", "-03", "+11", "-11", "+02", "-12", "Z"],
                [" +3", " -3", "+11", "-11", " +2", "-12", " +0"],
                [" +3", " -3", "+11", "-11", " +2", "-12", "Z"],
                ["+3", "-3", "+11", "-11", "+2", "-12", "+0"],
                ["+3", "-3", "+11", "-11", "+2", "-12", "Z"],
                ["+03", "-03", "+11", "-11", "+02", "-12", "+00"],
                ["+03", "-03", "+11", "-11", "+02", "-12", "Z"],
                [" +3", " -3", "+11", "-11", " +2", "-12", " +0"],
                [" +3", " -3", "+11", "-11", " +2", "-12", "Z"],
                ["+3", "-3", "+11", "-11", "+2", "-12", "+0"],
                ["+3", "-3", "+11", "-11", "+2", "-12", "Z"],
            ],
        );
        check_all(
            OffsetPrecision::Minutes,
            [
                ["+03:45", "-03:30", "+11:00", "-11:00", "+02:34", "-12:35", "+00:00"],
                ["+03:45", "-03:30", "+11:00", "-11:00", "+02:34", "-12:35", "Z"],
                [" +3:45", " -3:30", "+11:00", "-11:00", " +2:34", "-12:35", " +0:00"],
                [" +3:45", " -3:30", "+11:00", "-11:00", " +2:34", "-12:35", "Z"],
                ["+3:45", "-3:30", "+11:00", "-11:00", "+2:34", "-12:35", "+0:00"],
                ["+3:45", "-3:30", "+11:00", "-11:00", "+2:34", "-12:35", "Z"],
                ["+0345", "-0330", "+1100", "-1100", "+0234", "-1235", "+0000"],
                ["+0345", "-0330", "+1100", "-1100", "+0234", "-1235", "Z"],
                [" +345", " -330", "+1100", "-1100", " +234", "-1235", " +000"],
                [" +345", " -330", "+1100", "-1100", " +234", "-1235", "Z"],
                ["+345", "-330", "+1100", "-1100", "+234", "-1235", "+000"],
                ["+345", "-330", "+1100", "-1100", "+234", "-1235", "Z"],
            ],
        );
        #[rustfmt::skip]
        check_all(
            OffsetPrecision::Seconds,
            [
                ["+03:45:00", "-03:30:00", "+11:00:00", "-11:00:22", "+02:34:26", "-12:34:30", "+00:00:00"],
                ["+03:45:00", "-03:30:00", "+11:00:00", "-11:00:22", "+02:34:26", "-12:34:30", "Z"],
                [" +3:45:00", " -3:30:00", "+11:00:00", "-11:00:22", " +2:34:26", "-12:34:30", " +0:00:00"],
                [" +3:45:00", " -3:30:00", "+11:00:00", "-11:00:22", " +2:34:26", "-12:34:30", "Z"],
                ["+3:45:00", "-3:30:00", "+11:00:00", "-11:00:22", "+2:34:26", "-12:34:30", "+0:00:00"],
                ["+3:45:00", "-3:30:00", "+11:00:00", "-11:00:22", "+2:34:26", "-12:34:30", "Z"],
                ["+034500", "-033000", "+110000", "-110022", "+023426", "-123430", "+000000"],
                ["+034500", "-033000", "+110000", "-110022", "+023426", "-123430", "Z"],
                [" +34500", " -33000", "+110000", "-110022", " +23426", "-123430", " +00000"],
                [" +34500", " -33000", "+110000", "-110022", " +23426", "-123430", "Z"],
                ["+34500", "-33000", "+110000", "-110022", "+23426", "-123430", "+00000"],
                ["+34500", "-33000", "+110000", "-110022", "+23426", "-123430", "Z"],
            ],
        );
        check_all(
            OffsetPrecision::OptionalMinutes,
            [
                ["+03:45", "-03:30", "+11", "-11", "+02:34", "-12:35", "+00"],
                ["+03:45", "-03:30", "+11", "-11", "+02:34", "-12:35", "Z"],
                [" +3:45", " -3:30", "+11", "-11", " +2:34", "-12:35", " +0"],
                [" +3:45", " -3:30", "+11", "-11", " +2:34", "-12:35", "Z"],
                ["+3:45", "-3:30", "+11", "-11", "+2:34", "-12:35", "+0"],
                ["+3:45", "-3:30", "+11", "-11", "+2:34", "-12:35", "Z"],
                ["+0345", "-0330", "+11", "-11", "+0234", "-1235", "+00"],
                ["+0345", "-0330", "+11", "-11", "+0234", "-1235", "Z"],
                [" +345", " -330", "+11", "-11", " +234", "-1235", " +0"],
                [" +345", " -330", "+11", "-11", " +234", "-1235", "Z"],
                ["+345", "-330", "+11", "-11", "+234", "-1235", "+0"],
                ["+345", "-330", "+11", "-11", "+234", "-1235", "Z"],
            ],
        );
        check_all(
            OffsetPrecision::OptionalSeconds,
            [
                ["+03:45", "-03:30", "+11:00", "-11:00:22", "+02:34:26", "-12:34:30", "+00:00"],
                ["+03:45", "-03:30", "+11:00", "-11:00:22", "+02:34:26", "-12:34:30", "Z"],
                [" +3:45", " -3:30", "+11:00", "-11:00:22", " +2:34:26", "-12:34:30", " +0:00"],
                [" +3:45", " -3:30", "+11:00", "-11:00:22", " +2:34:26", "-12:34:30", "Z"],
                ["+3:45", "-3:30", "+11:00", "-11:00:22", "+2:34:26", "-12:34:30", "+0:00"],
                ["+3:45", "-3:30", "+11:00", "-11:00:22", "+2:34:26", "-12:34:30", "Z"],
                ["+0345", "-0330", "+1100", "-110022", "+023426", "-123430", "+0000"],
                ["+0345", "-0330", "+1100", "-110022", "+023426", "-123430", "Z"],
                [" +345", " -330", "+1100", "-110022", " +23426", "-123430", " +000"],
                [" +345", " -330", "+1100", "-110022", " +23426", "-123430", "Z"],
                ["+345", "-330", "+1100", "-110022", "+23426", "-123430", "+000"],
                ["+345", "-330", "+1100", "-110022", "+23426", "-123430", "Z"],
            ],
        );
        check_all(
            OffsetPrecision::OptionalMinutesAndSeconds,
            [
                ["+03:45", "-03:30", "+11", "-11:00:22", "+02:34:26", "-12:34:30", "+00"],
                ["+03:45", "-03:30", "+11", "-11:00:22", "+02:34:26", "-12:34:30", "Z"],
                [" +3:45", " -3:30", "+11", "-11:00:22", " +2:34:26", "-12:34:30", " +0"],
                [" +3:45", " -3:30", "+11", "-11:00:22", " +2:34:26", "-12:34:30", "Z"],
                ["+3:45", "-3:30", "+11", "-11:00:22", "+2:34:26", "-12:34:30", "+0"],
                ["+3:45", "-3:30", "+11", "-11:00:22", "+2:34:26", "-12:34:30", "Z"],
                ["+0345", "-0330", "+11", "-110022", "+023426", "-123430", "+00"],
                ["+0345", "-0330", "+11", "-110022", "+023426", "-123430", "Z"],
                [" +345", " -330", "+11", "-110022", " +23426", "-123430", " +0"],
                [" +345", " -330", "+11", "-110022", " +23426", "-123430", "Z"],
                ["+345", "-330", "+11", "-110022", "+23426", "-123430", "+0"],
                ["+345", "-330", "+11", "-110022", "+23426", "-123430", "Z"],
            ],
        );
    }
}
