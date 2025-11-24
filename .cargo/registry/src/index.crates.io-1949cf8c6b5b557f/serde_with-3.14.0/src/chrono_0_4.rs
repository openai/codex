//! De/Serialization of [chrono] types
//!
//! This modules is only available if using the `chrono_0_4` feature of the crate.
//!
//! [chrono]: https://docs.rs/chrono/

// Serialization of large numbers can result in overflows
// The time calculations are prone to this, so lint here extra
// https://github.com/jonasbb/serde_with/issues/771
#![warn(clippy::as_conversions)]

use crate::{
    formats::{Flexible, Format, Strict, Strictness},
    prelude::*,
};
#[cfg(feature = "std")]
use ::chrono_0_4::Local;
use ::chrono_0_4::{DateTime, Duration, NaiveDateTime, TimeZone, Utc};

/// Create a [`DateTime`] for the Unix Epoch using the [`Utc`] timezone
fn unix_epoch_utc() -> DateTime<Utc> {
    Utc.from_utc_datetime(&unix_epoch_naive())
}

/// Create a [`DateTime`] for the Unix Epoch using the [`Local`] timezone
#[cfg(feature = "std")]
fn unix_epoch_local() -> DateTime<Local> {
    Local.from_utc_datetime(&unix_epoch_naive())
}

/// Create a [`NaiveDateTime`] for the Unix Epoch
fn unix_epoch_naive() -> NaiveDateTime {
    DateTime::from_timestamp(0, 0).unwrap().naive_utc()
}

/// Deserialize a Unix timestamp with optional sub-second precision into a `DateTime<Utc>`.
///
/// The `DateTime<Utc>` can be serialized from an integer, a float, or a string representing a number.
///
/// # Examples
///
/// ```
/// # use chrono_0_4::{DateTime, Utc};
/// # use serde::Deserialize;
/// #
/// #[derive(Debug, Deserialize)]
/// struct S {
///     #[serde(with = "serde_with::chrono_0_4::datetime_utc_ts_seconds_from_any")]
///     date: DateTime<Utc>,
/// }
///
/// // Deserializes integers
/// assert!(serde_json::from_str::<S>(r#"{ "date": 1478563200 }"#).is_ok());
/// # // Ensure the date field is not dead code
/// # assert_eq!(serde_json::from_str::<S>(r#"{ "date": 1478563200 }"#).unwrap().date.timestamp(), 1478563200);
/// // floats
/// assert!(serde_json::from_str::<S>(r#"{ "date": 1478563200.123 }"#).is_ok());
/// // and strings with numbers, for high-precision values
/// assert!(serde_json::from_str::<S>(r#"{ "date": "1478563200.123" }"#).is_ok());
/// ```
// Requires float operations from std
#[cfg(feature = "std")]
pub mod datetime_utc_ts_seconds_from_any {
    use super::*;

    /// Deserialize a Unix timestamp with optional subsecond precision into a `DateTime<Utc>`.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper;
        impl Visitor<'_> for Helper {
            type Value = DateTime<Utc>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .write_str("an integer, float, or string with optional subsecond precision.")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                DateTime::from_timestamp(value, 0).ok_or_else(|| {
                    DeError::custom(format_args!(
                        "a timestamp which can be represented in a DateTime but received '{value}'"
                    ))
                })
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                let value = i64::try_from(value).map_err(|_| {
                    DeError::custom(format_args!(
                        "a timestamp which can be represented in a DateTime but received '{value}'"
                    ))
                })?;
                DateTime::from_timestamp(value, 0).ok_or_else(|| {
                    DeError::custom(format_args!(
                        "a timestamp which can be represented in a DateTime but received '{value}'"
                    ))
                })
            }

            // as conversions are necessary for floats
            #[allow(clippy::as_conversions)]
            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                let seconds = value.trunc() as i64;
                let nsecs = (value.fract() * 1_000_000_000_f64).abs() as u32;
                DateTime::from_timestamp(seconds, nsecs).ok_or_else(|| {
                    DeError::custom(format_args!(
                        "a timestamp which can be represented in a DateTime but received '{value}'"
                    ))
                })
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                let parts: Vec<_> = value.split('.').collect();

                match *parts.as_slice() {
                    [seconds] => {
                        if let Ok(seconds) = seconds.parse() {
                            DateTime::from_timestamp(seconds, 0).ok_or_else(|| {
                                DeError::custom(format_args!(
                                    "a timestamp which can be represented in a DateTime but received '{value}'"
                                ))
                            })
                        } else {
                            Err(DeError::invalid_value(Unexpected::Str(value), &self))
                        }
                    }
                    [seconds, subseconds] => {
                        if let Ok(seconds) = seconds.parse() {
                            let subseclen =
                            match u32::try_from(subseconds.chars().count()) {
                                Ok(subseclen) if subseclen <= 9 => subseclen,
                                _ =>    return Err(DeError::custom(format_args!(
                                    "DateTimes only support nanosecond precision but '{value}' has more than 9 digits."
                                ))),
                            };

                            if let Ok(mut subseconds) = subseconds.parse() {
                                // convert subseconds to nanoseconds (10^-9), require 9 places for nanoseconds
                                subseconds *= 10u32.pow(9 - subseclen);
                                DateTime::from_timestamp(seconds, subseconds).ok_or_else(|| {
                                    DeError::custom(format_args!(
                                        "a timestamp which can be represented in a DateTime but received '{value}'"
                                    ))
                                })
                            } else {
                                Err(DeError::invalid_value(Unexpected::Str(value), &self))
                            }
                        } else {
                            Err(DeError::invalid_value(Unexpected::Str(value), &self))
                        }
                    }

                    _ => Err(DeError::invalid_value(Unexpected::Str(value), &self)),
                }
            }
        }

        deserializer.deserialize_any(Helper)
    }
}

impl SerializeAs<NaiveDateTime> for DateTime<Utc> {
    fn serialize_as<S>(source: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let datetime = Utc.from_utc_datetime(source);
        datetime.serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NaiveDateTime> for DateTime<Utc> {
    fn deserialize_as<D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        DateTime::<Utc>::deserialize(deserializer).map(|datetime| datetime.naive_utc())
    }
}

/// Convert a [`chrono_0_4::Duration`] into a [`DurationSigned`]
fn duration_into_duration_signed(dur: &Duration) -> DurationSigned {
    match dur.to_std() {
        Ok(dur) => DurationSigned::with_duration(Sign::Positive, dur),
        Err(_) => {
            if let Ok(dur) = (-*dur).to_std() {
                DurationSigned::with_duration(Sign::Negative, dur)
            } else {
                panic!("A chrono Duration should be convertible to a DurationSigned")
            }
        }
    }
}

/// Convert a [`DurationSigned`] into a [`chrono_0_4::Duration`]
fn duration_from_duration_signed<'de, D>(dur: DurationSigned) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let mut chrono_dur = match Duration::from_std(dur.duration) {
        Ok(dur) => dur,
        Err(msg) => {
            return Err(DeError::custom(format_args!(
                "Duration is outside of the representable range: {msg}"
            )))
        }
    };
    if dur.sign.is_negative() {
        chrono_dur = -chrono_dur;
    }
    Ok(chrono_dur)
}

macro_rules! use_duration_signed_ser {
    (
        $main_trait:ident $internal_trait:ident =>
        {
            $ty:ty; $converter:ident =>
            $({
                $format:ty, $strictness:ty =>
                $($tbound:ident: $bound:ident $(,)?)*
            })*
        }
    ) => {
        $(
            impl<$($tbound ,)*> SerializeAs<$ty> for $main_trait<$format, $strictness>
            where
                $($tbound: $bound,)*
            {
                fn serialize_as<S>(source: &$ty, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: Serializer,
                {
                    let dur: DurationSigned = $converter(source);
                    $internal_trait::<$format, $strictness>::serialize_as(
                        &dur,
                        serializer,
                    )
                }
            }
        )*
    };
    (
        $( $main_trait:ident $internal_trait:ident, )+ => $rest:tt
    ) => {
        $( use_duration_signed_ser!($main_trait $internal_trait => $rest); )+
    };
}

fn datetime_to_duration<TZ>(source: &DateTime<TZ>) -> DurationSigned
where
    TZ: TimeZone,
{
    duration_into_duration_signed(&source.clone().signed_duration_since(unix_epoch_utc()))
}

fn naive_datetime_to_duration(source: &NaiveDateTime) -> DurationSigned {
    duration_into_duration_signed(&source.signed_duration_since(unix_epoch_naive()))
}

use_duration_signed_ser!(
    DurationSeconds DurationSeconds,
    DurationMilliSeconds DurationMilliSeconds,
    DurationMicroSeconds DurationMicroSeconds,
    DurationNanoSeconds DurationNanoSeconds,
    => {
        Duration; duration_into_duration_signed =>
        {i64, STRICTNESS => STRICTNESS: Strictness}
        {f64, STRICTNESS => STRICTNESS: Strictness}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_ser!(
    DurationSeconds DurationSeconds,
    DurationMilliSeconds DurationMilliSeconds,
    DurationMicroSeconds DurationMicroSeconds,
    DurationNanoSeconds DurationNanoSeconds,
    => {
        Duration; duration_into_duration_signed =>
        {String, STRICTNESS => STRICTNESS: Strictness}
    }
);
use_duration_signed_ser!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        DateTime<TZ>; datetime_to_duration =>
        {i64, STRICTNESS => TZ: TimeZone, STRICTNESS: Strictness}
        {f64, STRICTNESS => TZ: TimeZone, STRICTNESS: Strictness}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_ser!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        DateTime<TZ>; datetime_to_duration =>
        {String, STRICTNESS => TZ: TimeZone, STRICTNESS: Strictness}
    }
);
use_duration_signed_ser!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        NaiveDateTime; naive_datetime_to_duration =>
        {i64, STRICTNESS => STRICTNESS: Strictness}
        {f64, STRICTNESS => STRICTNESS: Strictness}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_ser!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        NaiveDateTime; naive_datetime_to_duration =>
        {String, STRICTNESS => STRICTNESS: Strictness}
    }
);

// Duration/Timestamp WITH FRACTIONS
use_duration_signed_ser!(
    DurationSecondsWithFrac DurationSecondsWithFrac,
    DurationMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    DurationMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    DurationNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        Duration; duration_into_duration_signed =>
        {f64, STRICTNESS => STRICTNESS: Strictness}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_ser!(
    DurationSecondsWithFrac DurationSecondsWithFrac,
    DurationMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    DurationMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    DurationNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        Duration; duration_into_duration_signed =>
        {String, STRICTNESS => STRICTNESS: Strictness}
    }
);
use_duration_signed_ser!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        DateTime<TZ>; datetime_to_duration =>
        {f64, STRICTNESS => TZ: TimeZone, STRICTNESS: Strictness}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_ser!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        DateTime<TZ>; datetime_to_duration =>
        {String, STRICTNESS => TZ: TimeZone, STRICTNESS: Strictness}
    }
);
use_duration_signed_ser!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        NaiveDateTime; naive_datetime_to_duration =>
        {f64, STRICTNESS => STRICTNESS: Strictness}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_ser!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        NaiveDateTime; naive_datetime_to_duration =>
        {String, STRICTNESS => STRICTNESS: Strictness}
    }
);

macro_rules! use_duration_signed_de {
    (
        $main_trait:ident $internal_trait:ident =>
        {
            $ty:ty; $converter:ident =>
            $({
                $format:ty, $strictness:ty =>
                $($tbound:ident: $bound:ident)*
            })*
        }
    ) =>{
        $(
            impl<'de, $($tbound,)*> DeserializeAs<'de, $ty> for $main_trait<$format, $strictness>
            where
                $($tbound: $bound,)*
            {
                fn deserialize_as<D>(deserializer: D) -> Result<$ty, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    let dur: DurationSigned = $internal_trait::<$format, $strictness>::deserialize_as(deserializer)?;
                    $converter::<D>(dur)
                }
            }
        )*
    };
    (
        $( $main_trait:ident $internal_trait:ident, )+ => $rest:tt
    ) => {
        $( use_duration_signed_de!($main_trait $internal_trait => $rest); )+
    };
}

fn duration_to_datetime_utc<'de, D>(dur: DurationSigned) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(unix_epoch_utc() + duration_from_duration_signed::<D>(dur)?)
}

#[cfg(feature = "std")]
fn duration_to_datetime_local<'de, D>(dur: DurationSigned) -> Result<DateTime<Local>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(unix_epoch_local() + duration_from_duration_signed::<D>(dur)?)
}

fn duration_to_naive_datetime<'de, D>(dur: DurationSigned) -> Result<NaiveDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(unix_epoch_naive() + duration_from_duration_signed::<D>(dur)?)
}

// No subsecond precision
use_duration_signed_de!(
    DurationSeconds DurationSeconds,
    DurationMilliSeconds DurationMilliSeconds,
    DurationMicroSeconds DurationMicroSeconds,
    DurationNanoSeconds DurationNanoSeconds,
    => {
        Duration; duration_from_duration_signed =>
        {i64, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_de!(
    DurationSeconds DurationSeconds,
    DurationMilliSeconds DurationMilliSeconds,
    DurationMicroSeconds DurationMicroSeconds,
    DurationNanoSeconds DurationNanoSeconds,
    => {
        Duration; duration_from_duration_signed =>
        {String, Strict =>}
    }
);
#[cfg(feature = "std")]
use_duration_signed_de!(
    DurationSeconds DurationSeconds,
    DurationMilliSeconds DurationMilliSeconds,
    DurationMicroSeconds DurationMicroSeconds,
    DurationNanoSeconds DurationNanoSeconds,
    => {
        Duration; duration_from_duration_signed =>
        {f64, Strict =>}
    }
);
use_duration_signed_de!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        DateTime<Utc>; duration_to_datetime_utc =>
        {i64, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_de!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        DateTime<Utc>; duration_to_datetime_utc =>
        {String, Strict =>}
    }
);
#[cfg(feature = "std")]
use_duration_signed_de!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        DateTime<Utc>; duration_to_datetime_utc =>
        {f64, Strict =>}
    }
);
#[cfg(feature = "std")]
use_duration_signed_de!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        DateTime<Local>; duration_to_datetime_local =>
        {i64, Strict =>}
        {f64, Strict =>}
        {String, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);
use_duration_signed_de!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        NaiveDateTime; duration_to_naive_datetime =>
        {i64, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_de!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        NaiveDateTime; duration_to_naive_datetime =>
        {String, Strict =>}
    }
);
#[cfg(feature = "std")]
use_duration_signed_de!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        NaiveDateTime; duration_to_naive_datetime =>
        {f64, Strict =>}
    }
);

// Duration/Timestamp WITH FRACTIONS
use_duration_signed_de!(
    DurationSecondsWithFrac DurationSecondsWithFrac,
    DurationMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    DurationMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    DurationNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        Duration; duration_from_duration_signed =>
        {f64, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_de!(
    DurationSecondsWithFrac DurationSecondsWithFrac,
    DurationMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    DurationMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    DurationNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        Duration; duration_from_duration_signed =>
        {String, Strict =>}
    }
);
use_duration_signed_de!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        DateTime<Utc>; duration_to_datetime_utc =>
        {f64, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_de!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        DateTime<Utc>; duration_to_datetime_utc =>
        {String, Strict =>}
    }
);
#[cfg(feature = "std")]
use_duration_signed_de!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        DateTime<Local>; duration_to_datetime_local =>
        {f64, Strict =>}
        {String, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);
use_duration_signed_de!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        NaiveDateTime; duration_to_naive_datetime =>
        {f64, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);
#[cfg(feature = "alloc")]
use_duration_signed_de!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        NaiveDateTime; duration_to_naive_datetime =>
        {String, Strict =>}
    }
);
