//! Serde visitor for various types.

use core::fmt;
use core::marker::PhantomData;

use serde::de;
#[cfg(feature = "parsing")]
use serde::Deserializer;

#[cfg(feature = "parsing")]
use super::{
    DATE_FORMAT, OFFSET_DATE_TIME_FORMAT, PRIMITIVE_DATE_TIME_FORMAT, TIME_FORMAT,
    UTC_DATE_TIME_FORMAT, UTC_OFFSET_FORMAT,
};
use crate::error::ComponentRange;
#[cfg(feature = "parsing")]
use crate::format_description::well_known::*;
use crate::{
    Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcDateTime, UtcOffset, Weekday,
};

/// A serde visitor for various types.
pub(super) struct Visitor<T: ?Sized>(pub(super) PhantomData<T>);

impl<'a> de::Visitor<'a> for Visitor<Date> {
    type Value = Date;

    #[inline]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a `Date`")
    }

    #[cfg(feature = "parsing")]
    #[inline]
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Date, E> {
        Date::parse(value, &DATE_FORMAT).map_err(E::custom)
    }

    #[inline]
    fn visit_seq<A: de::SeqAccess<'a>>(self, mut seq: A) -> Result<Date, A::Error> {
        let year = item!(seq, "year")?;
        let ordinal = item!(seq, "day of year")?;
        Date::from_ordinal_date(year, ordinal).map_err(ComponentRange::into_de_error)
    }
}

impl<'a> de::Visitor<'a> for Visitor<Duration> {
    type Value = Duration;

    #[inline]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a `Duration`")
    }

    #[inline]
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Duration, E> {
        let (seconds, nanoseconds) = value.split_once('.').ok_or_else(|| {
            de::Error::invalid_value(de::Unexpected::Str(value), &"a decimal point")
        })?;

        let seconds = seconds
            .parse()
            .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(seconds), &"seconds"))?;
        let mut nanoseconds = nanoseconds.parse().map_err(|_| {
            de::Error::invalid_value(de::Unexpected::Str(nanoseconds), &"nanoseconds")
        })?;

        if seconds < 0
            // make sure sign does not disappear when seconds == 0
            || (seconds == 0 && value.starts_with("-"))
        {
            nanoseconds *= -1;
        }

        Ok(Duration::new(seconds, nanoseconds))
    }

    #[inline]
    fn visit_seq<A: de::SeqAccess<'a>>(self, mut seq: A) -> Result<Duration, A::Error> {
        let seconds = item!(seq, "seconds")?;
        let nanoseconds = item!(seq, "nanoseconds")?;
        Ok(Duration::new(seconds, nanoseconds))
    }
}

impl<'a> de::Visitor<'a> for Visitor<OffsetDateTime> {
    type Value = OffsetDateTime;

    #[inline]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an `OffsetDateTime`")
    }

    #[cfg(feature = "parsing")]
    #[inline]
    fn visit_str<E: de::Error>(self, value: &str) -> Result<OffsetDateTime, E> {
        OffsetDateTime::parse(value, &OFFSET_DATE_TIME_FORMAT).map_err(E::custom)
    }

    #[inline]
    fn visit_seq<A: de::SeqAccess<'a>>(self, mut seq: A) -> Result<OffsetDateTime, A::Error> {
        let year = item!(seq, "year")?;
        let ordinal = item!(seq, "day of year")?;
        let hour = item!(seq, "hour")?;
        let minute = item!(seq, "minute")?;
        let second = item!(seq, "second")?;
        let nanosecond = item!(seq, "nanosecond")?;
        let offset_hours = item!(seq, "offset hours")?;
        let offset_minutes = item!(seq, "offset minutes")?;
        let offset_seconds = item!(seq, "offset seconds")?;

        Date::from_ordinal_date(year, ordinal)
            .and_then(|date| date.with_hms_nano(hour, minute, second, nanosecond))
            .and_then(|datetime| {
                UtcOffset::from_hms(offset_hours, offset_minutes, offset_seconds)
                    .map(|offset| datetime.assume_offset(offset))
            })
            .map_err(ComponentRange::into_de_error)
    }
}

impl<'a> de::Visitor<'a> for Visitor<PrimitiveDateTime> {
    type Value = PrimitiveDateTime;

    #[inline]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a `PrimitiveDateTime`")
    }

    #[cfg(feature = "parsing")]
    #[inline]
    fn visit_str<E: de::Error>(self, value: &str) -> Result<PrimitiveDateTime, E> {
        PrimitiveDateTime::parse(value, &PRIMITIVE_DATE_TIME_FORMAT).map_err(E::custom)
    }

    #[inline]
    fn visit_seq<A: de::SeqAccess<'a>>(self, mut seq: A) -> Result<PrimitiveDateTime, A::Error> {
        let year = item!(seq, "year")?;
        let ordinal = item!(seq, "day of year")?;
        let hour = item!(seq, "hour")?;
        let minute = item!(seq, "minute")?;
        let second = item!(seq, "second")?;
        let nanosecond = item!(seq, "nanosecond")?;

        Date::from_ordinal_date(year, ordinal)
            .and_then(|date| date.with_hms_nano(hour, minute, second, nanosecond))
            .map_err(ComponentRange::into_de_error)
    }
}

impl<'a> de::Visitor<'a> for Visitor<UtcDateTime> {
    type Value = UtcDateTime;

    #[inline]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a `PrimitiveDateTime`")
    }

    #[cfg(feature = "parsing")]
    #[inline]
    fn visit_str<E: de::Error>(self, value: &str) -> Result<UtcDateTime, E> {
        UtcDateTime::parse(value, &UTC_DATE_TIME_FORMAT).map_err(E::custom)
    }

    #[inline]
    fn visit_seq<A: de::SeqAccess<'a>>(self, mut seq: A) -> Result<UtcDateTime, A::Error> {
        let year = item!(seq, "year")?;
        let ordinal = item!(seq, "day of year")?;
        let hour = item!(seq, "hour")?;
        let minute = item!(seq, "minute")?;
        let second = item!(seq, "second")?;
        let nanosecond = item!(seq, "nanosecond")?;

        Date::from_ordinal_date(year, ordinal)
            .and_then(|date| date.with_hms_nano(hour, minute, second, nanosecond))
            .map(UtcDateTime::from_primitive)
            .map_err(ComponentRange::into_de_error)
    }
}

impl<'a> de::Visitor<'a> for Visitor<Time> {
    type Value = Time;

    #[inline]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a `Time`")
    }

    #[cfg(feature = "parsing")]
    #[inline]
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Time, E> {
        Time::parse(value, &TIME_FORMAT).map_err(E::custom)
    }

    #[inline]
    fn visit_seq<A: de::SeqAccess<'a>>(self, mut seq: A) -> Result<Time, A::Error> {
        let hour = item!(seq, "hour")?;
        let minute = item!(seq, "minute")?;
        let second = item!(seq, "second")?;
        let nanosecond = item!(seq, "nanosecond")?;

        Time::from_hms_nano(hour, minute, second, nanosecond).map_err(ComponentRange::into_de_error)
    }
}

impl<'a> de::Visitor<'a> for Visitor<UtcOffset> {
    type Value = UtcOffset;

    #[inline]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a `UtcOffset`")
    }

    #[cfg(feature = "parsing")]
    #[inline]
    fn visit_str<E: de::Error>(self, value: &str) -> Result<UtcOffset, E> {
        UtcOffset::parse(value, &UTC_OFFSET_FORMAT).map_err(E::custom)
    }

    #[inline]
    fn visit_seq<A: de::SeqAccess<'a>>(self, mut seq: A) -> Result<UtcOffset, A::Error> {
        let hours = item!(seq, "offset hours")?;
        let mut minutes = 0;
        let mut seconds = 0;

        if let Ok(Some(min)) = seq.next_element() {
            minutes = min;
            if let Ok(Some(sec)) = seq.next_element() {
                seconds = sec;
            }
        };

        UtcOffset::from_hms(hours, minutes, seconds).map_err(ComponentRange::into_de_error)
    }
}

impl de::Visitor<'_> for Visitor<Weekday> {
    type Value = Weekday;

    #[inline]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a `Weekday`")
    }

    #[inline]
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Weekday, E> {
        match value {
            "Monday" => Ok(Weekday::Monday),
            "Tuesday" => Ok(Weekday::Tuesday),
            "Wednesday" => Ok(Weekday::Wednesday),
            "Thursday" => Ok(Weekday::Thursday),
            "Friday" => Ok(Weekday::Friday),
            "Saturday" => Ok(Weekday::Saturday),
            "Sunday" => Ok(Weekday::Sunday),
            _ => Err(E::invalid_value(de::Unexpected::Str(value), &"a `Weekday`")),
        }
    }

    #[inline]
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Weekday, E> {
        match value {
            1 => Ok(Weekday::Monday),
            2 => Ok(Weekday::Tuesday),
            3 => Ok(Weekday::Wednesday),
            4 => Ok(Weekday::Thursday),
            5 => Ok(Weekday::Friday),
            6 => Ok(Weekday::Saturday),
            7 => Ok(Weekday::Sunday),
            _ => Err(E::invalid_value(
                de::Unexpected::Unsigned(value),
                &"a value in the range 1..=7",
            )),
        }
    }
}

impl de::Visitor<'_> for Visitor<Month> {
    type Value = Month;

    #[inline]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a `Month`")
    }

    #[inline]
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Month, E> {
        match value {
            "January" => Ok(Month::January),
            "February" => Ok(Month::February),
            "March" => Ok(Month::March),
            "April" => Ok(Month::April),
            "May" => Ok(Month::May),
            "June" => Ok(Month::June),
            "July" => Ok(Month::July),
            "August" => Ok(Month::August),
            "September" => Ok(Month::September),
            "October" => Ok(Month::October),
            "November" => Ok(Month::November),
            "December" => Ok(Month::December),
            _ => Err(E::invalid_value(de::Unexpected::Str(value), &"a `Month`")),
        }
    }

    #[inline]
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Month, E> {
        match value {
            1 => Ok(Month::January),
            2 => Ok(Month::February),
            3 => Ok(Month::March),
            4 => Ok(Month::April),
            5 => Ok(Month::May),
            6 => Ok(Month::June),
            7 => Ok(Month::July),
            8 => Ok(Month::August),
            9 => Ok(Month::September),
            10 => Ok(Month::October),
            11 => Ok(Month::November),
            12 => Ok(Month::December),
            _ => Err(E::invalid_value(
                de::Unexpected::Unsigned(value),
                &"a value in the range 1..=12",
            )),
        }
    }
}

/// Implement a visitor for a well-known format.
macro_rules! well_known {
    ($article:literal, $name:literal, $($ty:tt)+) => {
        #[cfg(feature = "parsing")]
        impl de::Visitor<'_> for Visitor<$($ty)+> {
            type Value = OffsetDateTime;

            #[inline]
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!($article, " ", $name, "-formatted `OffsetDateTime`"))
            }

            #[inline]
            fn visit_str<E: de::Error>(self, value: &str) -> Result<OffsetDateTime, E> {
                OffsetDateTime::parse(value, &$($ty)+).map_err(E::custom)
            }
        }

        #[cfg(feature = "parsing")]
        impl<'a> de::Visitor<'a> for Visitor<Option<$($ty)+>> {
            type Value = Option<OffsetDateTime>;

            #[inline]
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(concat!(
                    $article,
                    " ",
                    $name,
                    "-formatted `Option<OffsetDateTime>`"
                ))
            }

            #[inline]
            fn visit_some<D: Deserializer<'a>>(
                self,
                deserializer: D,
            ) -> Result<Option<OffsetDateTime>, D::Error> {
                deserializer
                    .deserialize_any(Visitor::<$($ty)+>(PhantomData))
                    .map(Some)
            }

            #[inline]
            fn visit_none<E: de::Error>(self) -> Result<Option<OffsetDateTime>, E> {
                Ok(None)
            }

            #[inline]
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(None)
            }
        }
    };
}

well_known!("an", "RFC2822", Rfc2822);
well_known!("an", "RFC3339", Rfc3339);
well_known!(
    "an",
    "ISO 8601",
    Iso8601::<{ super::iso8601::SERDE_CONFIG }>
);
