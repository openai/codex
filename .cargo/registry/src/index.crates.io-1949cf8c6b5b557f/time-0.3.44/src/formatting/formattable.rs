//! A trait that can be used to format an item from its components.

use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Deref;
use std::io;

use num_conv::prelude::*;

use crate::format_description::well_known::iso8601::EncodedConfig;
use crate::format_description::well_known::{Iso8601, Rfc2822, Rfc3339};
use crate::format_description::{BorrowedFormatItem, OwnedFormatItem};
use crate::formatting::{
    format_component, format_number_pad_zero, iso8601, write, MONTH_NAMES, WEEKDAY_NAMES,
};
use crate::{error, Date, Time, UtcOffset};

/// A type that describes a format.
///
/// Implementors of [`Formattable`] are [format descriptions](crate::format_description).
///
/// [`Date::format`] and [`Time::format`] each use a format description to generate
/// a String from their data. See the respective methods for usage examples.
#[cfg_attr(docsrs, doc(notable_trait))]
pub trait Formattable: sealed::Sealed {}
impl Formattable for BorrowedFormatItem<'_> {}
impl Formattable for [BorrowedFormatItem<'_>] {}
impl Formattable for OwnedFormatItem {}
impl Formattable for [OwnedFormatItem] {}
impl Formattable for Rfc3339 {}
impl Formattable for Rfc2822 {}
impl<const CONFIG: EncodedConfig> Formattable for Iso8601<CONFIG> {}
impl<T: Deref> Formattable for T where T::Target: Formattable {}

/// Seal the trait to prevent downstream users from implementing it.
mod sealed {
    use super::*;

    /// Format the item using a format description, the intended output, and the various components.
    pub trait Sealed {
        /// Format the item into the provided output, returning the number of bytes written.
        fn format_into(
            &self,
            output: &mut (impl io::Write + ?Sized),
            date: Option<Date>,
            time: Option<Time>,
            offset: Option<UtcOffset>,
        ) -> Result<usize, error::Format>;

        /// Format the item directly to a `String`.
        #[inline]
        fn format(
            &self,
            date: Option<Date>,
            time: Option<Time>,
            offset: Option<UtcOffset>,
        ) -> Result<String, error::Format> {
            let mut buf = Vec::new();
            self.format_into(&mut buf, date, time, offset)?;
            Ok(String::from_utf8_lossy(&buf).into_owned())
        }
    }
}

impl sealed::Sealed for BorrowedFormatItem<'_> {
    #[inline]
    fn format_into(
        &self,
        output: &mut (impl io::Write + ?Sized),
        date: Option<Date>,
        time: Option<Time>,
        offset: Option<UtcOffset>,
    ) -> Result<usize, error::Format> {
        Ok(match *self {
            Self::Literal(literal) => write(output, literal)?,
            Self::Component(component) => format_component(output, component, date, time, offset)?,
            Self::Compound(items) => items.format_into(output, date, time, offset)?,
            Self::Optional(item) => item.format_into(output, date, time, offset)?,
            Self::First(items) => match items {
                [] => 0,
                [item, ..] => item.format_into(output, date, time, offset)?,
            },
        })
    }
}

impl sealed::Sealed for [BorrowedFormatItem<'_>] {
    #[inline]
    fn format_into(
        &self,
        output: &mut (impl io::Write + ?Sized),
        date: Option<Date>,
        time: Option<Time>,
        offset: Option<UtcOffset>,
    ) -> Result<usize, error::Format> {
        let mut bytes = 0;
        for item in self.iter() {
            bytes += item.format_into(output, date, time, offset)?;
        }
        Ok(bytes)
    }
}

impl sealed::Sealed for OwnedFormatItem {
    #[inline]
    fn format_into(
        &self,
        output: &mut (impl io::Write + ?Sized),
        date: Option<Date>,
        time: Option<Time>,
        offset: Option<UtcOffset>,
    ) -> Result<usize, error::Format> {
        match self {
            Self::Literal(literal) => Ok(write(output, literal)?),
            Self::Component(component) => format_component(output, *component, date, time, offset),
            Self::Compound(items) => items.format_into(output, date, time, offset),
            Self::Optional(item) => item.format_into(output, date, time, offset),
            Self::First(items) => match &**items {
                [] => Ok(0),
                [item, ..] => item.format_into(output, date, time, offset),
            },
        }
    }
}

impl sealed::Sealed for [OwnedFormatItem] {
    #[inline]
    fn format_into(
        &self,
        output: &mut (impl io::Write + ?Sized),
        date: Option<Date>,
        time: Option<Time>,
        offset: Option<UtcOffset>,
    ) -> Result<usize, error::Format> {
        let mut bytes = 0;
        for item in self.iter() {
            bytes += item.format_into(output, date, time, offset)?;
        }
        Ok(bytes)
    }
}

impl<T> sealed::Sealed for T
where
    T: Deref<Target: sealed::Sealed>,
{
    #[inline]
    fn format_into(
        &self,
        output: &mut (impl io::Write + ?Sized),
        date: Option<Date>,
        time: Option<Time>,
        offset: Option<UtcOffset>,
    ) -> Result<usize, error::Format> {
        self.deref().format_into(output, date, time, offset)
    }
}

impl sealed::Sealed for Rfc2822 {
    fn format_into(
        &self,
        output: &mut (impl io::Write + ?Sized),
        date: Option<Date>,
        time: Option<Time>,
        offset: Option<UtcOffset>,
    ) -> Result<usize, error::Format> {
        let date = date.ok_or(error::Format::InsufficientTypeInformation)?;
        let time = time.ok_or(error::Format::InsufficientTypeInformation)?;
        let offset = offset.ok_or(error::Format::InsufficientTypeInformation)?;

        let mut bytes = 0;

        let (year, month, day) = date.to_calendar_date();

        if year < 1900 {
            return Err(error::Format::InvalidComponent("year"));
        }
        if offset.seconds_past_minute() != 0 {
            return Err(error::Format::InvalidComponent("offset_second"));
        }

        bytes += write(
            output,
            &WEEKDAY_NAMES[date.weekday().number_days_from_monday().extend::<usize>()][..3],
        )?;
        bytes += write(output, b", ")?;
        bytes += format_number_pad_zero::<2>(output, day)?;
        bytes += write(output, b" ")?;
        bytes += write(
            output,
            &MONTH_NAMES[u8::from(month).extend::<usize>() - 1][..3],
        )?;
        bytes += write(output, b" ")?;
        bytes += format_number_pad_zero::<4>(output, year.cast_unsigned())?;
        bytes += write(output, b" ")?;
        bytes += format_number_pad_zero::<2>(output, time.hour())?;
        bytes += write(output, b":")?;
        bytes += format_number_pad_zero::<2>(output, time.minute())?;
        bytes += write(output, b":")?;
        bytes += format_number_pad_zero::<2>(output, time.second())?;
        bytes += write(output, b" ")?;
        bytes += write(output, if offset.is_negative() { b"-" } else { b"+" })?;
        bytes += format_number_pad_zero::<2>(output, offset.whole_hours().unsigned_abs())?;
        bytes += format_number_pad_zero::<2>(output, offset.minutes_past_hour().unsigned_abs())?;

        Ok(bytes)
    }
}

impl sealed::Sealed for Rfc3339 {
    fn format_into(
        &self,
        output: &mut (impl io::Write + ?Sized),
        date: Option<Date>,
        time: Option<Time>,
        offset: Option<UtcOffset>,
    ) -> Result<usize, error::Format> {
        let date = date.ok_or(error::Format::InsufficientTypeInformation)?;
        let time = time.ok_or(error::Format::InsufficientTypeInformation)?;
        let offset = offset.ok_or(error::Format::InsufficientTypeInformation)?;

        let mut bytes = 0;

        let year = date.year();

        if !(0..10_000).contains(&year) {
            return Err(error::Format::InvalidComponent("year"));
        }
        if offset.whole_hours().unsigned_abs() > 23 {
            return Err(error::Format::InvalidComponent("offset_hour"));
        }
        if offset.seconds_past_minute() != 0 {
            return Err(error::Format::InvalidComponent("offset_second"));
        }

        bytes += format_number_pad_zero::<4>(output, year.cast_unsigned())?;
        bytes += write(output, b"-")?;
        bytes += format_number_pad_zero::<2>(output, u8::from(date.month()))?;
        bytes += write(output, b"-")?;
        bytes += format_number_pad_zero::<2>(output, date.day())?;
        bytes += write(output, b"T")?;
        bytes += format_number_pad_zero::<2>(output, time.hour())?;
        bytes += write(output, b":")?;
        bytes += format_number_pad_zero::<2>(output, time.minute())?;
        bytes += write(output, b":")?;
        bytes += format_number_pad_zero::<2>(output, time.second())?;

        if time.nanosecond() != 0 {
            let nanos = time.nanosecond();
            bytes += write(output, b".")?;
            bytes += if nanos % 10 != 0 {
                format_number_pad_zero::<9>(output, nanos)
            } else if (nanos / 10) % 10 != 0 {
                format_number_pad_zero::<8>(output, nanos / 10)
            } else if (nanos / 100) % 10 != 0 {
                format_number_pad_zero::<7>(output, nanos / 100)
            } else if (nanos / 1_000) % 10 != 0 {
                format_number_pad_zero::<6>(output, nanos / 1_000)
            } else if (nanos / 10_000) % 10 != 0 {
                format_number_pad_zero::<5>(output, nanos / 10_000)
            } else if (nanos / 100_000) % 10 != 0 {
                format_number_pad_zero::<4>(output, nanos / 100_000)
            } else if (nanos / 1_000_000) % 10 != 0 {
                format_number_pad_zero::<3>(output, nanos / 1_000_000)
            } else if (nanos / 10_000_000) % 10 != 0 {
                format_number_pad_zero::<2>(output, nanos / 10_000_000)
            } else {
                format_number_pad_zero::<1>(output, nanos / 100_000_000)
            }?;
        }

        if offset == UtcOffset::UTC {
            bytes += write(output, b"Z")?;
            return Ok(bytes);
        }

        bytes += write(output, if offset.is_negative() { b"-" } else { b"+" })?;
        bytes += format_number_pad_zero::<2>(output, offset.whole_hours().unsigned_abs())?;
        bytes += write(output, b":")?;
        bytes += format_number_pad_zero::<2>(output, offset.minutes_past_hour().unsigned_abs())?;

        Ok(bytes)
    }
}

impl<const CONFIG: EncodedConfig> sealed::Sealed for Iso8601<CONFIG> {
    #[inline]
    fn format_into(
        &self,
        output: &mut (impl io::Write + ?Sized),
        date: Option<Date>,
        time: Option<Time>,
        offset: Option<UtcOffset>,
    ) -> Result<usize, error::Format> {
        let mut bytes = 0;

        if Self::FORMAT_DATE {
            let date = date.ok_or(error::Format::InsufficientTypeInformation)?;
            bytes += iso8601::format_date::<CONFIG>(output, date)?;
        }
        if Self::FORMAT_TIME {
            let time = time.ok_or(error::Format::InsufficientTypeInformation)?;
            bytes += iso8601::format_time::<CONFIG>(output, time)?;
        }
        if Self::FORMAT_OFFSET {
            let offset = offset.ok_or(error::Format::InsufficientTypeInformation)?;
            bytes += iso8601::format_offset::<CONFIG>(output, offset)?;
        }

        if bytes == 0 {
            // The only reason there would be no bytes written is if the format was only for
            // parsing.
            panic!("attempted to format a parsing-only format description");
        }

        Ok(bytes)
    }
}
