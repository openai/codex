use core::fmt;
use serde::{de, ser};

use super::DateTime;
use crate::format::{SecondsFormat, write_rfc3339};
#[cfg(feature = "clock")]
use crate::offset::Local;
use crate::offset::{FixedOffset, Offset, TimeZone, Utc};

#[doc(hidden)]
#[derive(Debug)]
pub struct SecondsTimestampVisitor;

#[doc(hidden)]
#[derive(Debug)]
pub struct NanoSecondsTimestampVisitor;

#[doc(hidden)]
#[derive(Debug)]
pub struct MicroSecondsTimestampVisitor;

#[doc(hidden)]
#[derive(Debug)]
pub struct MilliSecondsTimestampVisitor;

/// Serialize to an RFC 3339 formatted string
///
/// As an extension to RFC 3339 this can serialize `DateTime`s outside the range of 0-9999 years
/// using an ISO 8601 syntax (which prepends an `-` or `+`).
///
/// See [the `serde` module](crate::serde) for alternate serializations.
impl<Tz: TimeZone> ser::Serialize for DateTime<Tz> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        struct FormatIso8601<'a, Tz: TimeZone> {
            inner: &'a DateTime<Tz>,
        }

        impl<Tz: TimeZone> fmt::Display for FormatIso8601<'_, Tz> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let naive = self.inner.naive_local();
                let offset = self.inner.offset.fix();
                write_rfc3339(f, naive, offset, SecondsFormat::AutoSi, true)
            }
        }

        serializer.collect_str(&FormatIso8601 { inner: self })
    }
}

struct DateTimeVisitor;

impl de::Visitor<'_> for DateTimeVisitor {
    type Value = DateTime<FixedOffset>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an RFC 3339 formatted date and time string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

/// Deserialize an RFC 3339 formatted string into a `DateTime<FixedOffset>`
///
/// As an extension to RFC 3339 this can deserialize to `DateTime`s outside the range of 0-9999
/// years using an ISO 8601 syntax (which prepends an `-` or `+`).
///
/// See [the `serde` module](crate::serde) for alternate deserialization formats.
impl<'de> de::Deserialize<'de> for DateTime<FixedOffset> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_str(DateTimeVisitor)
    }
}

/// Deserialize an RFC 3339 formatted string into a `DateTime<Utc>`
///
/// If the value contains an offset from UTC that is not zero, the value will be converted to UTC.
///
/// As an extension to RFC 3339 this can deserialize to `DateTime`s outside the range of 0-9999
/// years using an ISO 8601 syntax (which prepends an `-` or `+`).
///
/// See [the `serde` module](crate::serde) for alternate deserialization formats.
impl<'de> de::Deserialize<'de> for DateTime<Utc> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_str(DateTimeVisitor).map(|dt| dt.with_timezone(&Utc))
    }
}

/// Deserialize an RFC 3339 formatted string into a `DateTime<Local>`
///
/// The value will remain the same instant in UTC, but the offset will be recalculated to match
/// that of the `Local` platform time zone.
///
/// As an extension to RFC 3339 this can deserialize to `DateTime`s outside the range of 0-9999
/// years using an ISO 8601 syntax (which prepends an `-` or `+`).
///
/// See [the `serde` module](crate::serde) for alternate deserialization formats.
#[cfg(feature = "clock")]
impl<'de> de::Deserialize<'de> for DateTime<Local> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_str(DateTimeVisitor).map(|dt| dt.with_timezone(&Local))
    }
}

/// Ser/de to/from timestamps in nanoseconds
///
/// Intended for use with `serde`'s `with` attribute.
///
/// # Example:
///
/// ```rust
/// # use chrono::{DateTime, Utc, NaiveDate};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::serde::ts_nanoseconds;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_nanoseconds")]
///     time: DateTime<Utc>,
/// }
///
/// let time = NaiveDate::from_ymd_opt(2018, 5, 17)
///     .unwrap()
///     .and_hms_nano_opt(02, 04, 59, 918355733)
///     .unwrap()
///     .and_utc();
/// let my_s = S { time: time.clone() };
///
/// let as_string = serde_json::to_string(&my_s)?;
/// assert_eq!(as_string, r#"{"time":1526522699918355733}"#);
/// let my_s: S = serde_json::from_str(&as_string)?;
/// assert_eq!(my_s.time, time);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub mod ts_nanoseconds {
    use core::fmt;
    use serde::{de, ser};

    use crate::serde::invalid_ts;
    use crate::{DateTime, Utc};

    use super::NanoSecondsTimestampVisitor;

    /// Serialize a UTC datetime into an integer number of nanoseconds since the epoch
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Errors
    ///
    /// An `i64` with nanosecond precision can span a range of ~584 years. This function returns an
    /// error on an out of range `DateTime`.
    ///
    /// The dates that can be represented as nanoseconds are between 1677-09-21T00:12:44.0 and
    /// 2262-04-11T23:47:16.854775804.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, Utc, NaiveDate};
    /// # use serde_derive::Serialize;
    /// use chrono::serde::ts_nanoseconds::serialize as to_nano_ts;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_nano_ts")]
    ///     time: DateTime<Utc>,
    /// }
    ///
    /// let my_s = S {
    ///     time: NaiveDate::from_ymd_opt(2018, 5, 17)
    ///         .unwrap()
    ///         .and_hms_nano_opt(02, 04, 59, 918355733)
    ///         .unwrap()
    ///         .and_utc(),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918355733}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.timestamp_nanos_opt().ok_or(ser::Error::custom(
            "value out of range for a timestamp with nanosecond precision",
        ))?)
    }

    /// Deserialize a [`DateTime`] from a nanosecond timestamp
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, TimeZone, Utc};
    /// # use serde_derive::Deserialize;
    /// use chrono::serde::ts_nanoseconds::deserialize as from_nano_ts;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_nano_ts")]
    ///     time: DateTime<Utc>,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1526522699918355733 }"#)?;
    /// assert_eq!(my_s, S { time: Utc.timestamp_opt(1526522699, 918355733).unwrap() });
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": -1 }"#)?;
    /// assert_eq!(my_s, S { time: Utc.timestamp_opt(-1, 999_999_999).unwrap() });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(NanoSecondsTimestampVisitor)
    }

    impl de::Visitor<'_> for NanoSecondsTimestampVisitor {
        type Value = DateTime<Utc>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in nanoseconds")
        }

        /// Deserialize a timestamp in nanoseconds since the epoch
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(DateTime::from_timestamp_nanos(value))
        }

        /// Deserialize a timestamp in nanoseconds since the epoch
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp((value / 1_000_000_000) as i64, (value % 1_000_000_000) as u32)
                .ok_or_else(|| invalid_ts(value))
        }
    }
}

/// Ser/de to/from optional timestamps in nanoseconds
///
/// Intended for use with `serde`'s `with` attribute.
///
/// # Example:
///
/// ```rust
/// # use chrono::{DateTime, Utc, NaiveDate};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::serde::ts_nanoseconds_option;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_nanoseconds_option")]
///     time: Option<DateTime<Utc>>,
/// }
///
/// let time = Some(
///     NaiveDate::from_ymd_opt(2018, 5, 17)
///         .unwrap()
///         .and_hms_nano_opt(02, 04, 59, 918355733)
///         .unwrap()
///         .and_utc(),
/// );
/// let my_s = S { time: time.clone() };
///
/// let as_string = serde_json::to_string(&my_s)?;
/// assert_eq!(as_string, r#"{"time":1526522699918355733}"#);
/// let my_s: S = serde_json::from_str(&as_string)?;
/// assert_eq!(my_s.time, time);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub mod ts_nanoseconds_option {
    use core::fmt;
    use serde::{de, ser};

    use crate::{DateTime, Utc};

    use super::NanoSecondsTimestampVisitor;

    /// Serialize a UTC datetime into an integer number of nanoseconds since the epoch or none
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Errors
    ///
    /// An `i64` with nanosecond precision can span a range of ~584 years. This function returns an
    /// error on an out of range `DateTime`.
    ///
    /// The dates that can be represented as nanoseconds are between 1677-09-21T00:12:44.0 and
    /// 2262-04-11T23:47:16.854775804.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, Utc, NaiveDate};
    /// # use serde_derive::Serialize;
    /// use chrono::serde::ts_nanoseconds_option::serialize as to_nano_tsopt;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_nano_tsopt")]
    ///     time: Option<DateTime<Utc>>,
    /// }
    ///
    /// let my_s = S {
    ///     time: Some(
    ///         NaiveDate::from_ymd_opt(2018, 5, 17)
    ///             .unwrap()
    ///             .and_hms_nano_opt(02, 04, 59, 918355733)
    ///             .unwrap()
    ///             .and_utc(),
    ///     ),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918355733}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(opt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.timestamp_nanos_opt().ok_or(
                ser::Error::custom("value out of range for a timestamp with nanosecond precision"),
            )?),
            None => serializer.serialize_none(),
        }
    }

    /// Deserialize a `DateTime` from a nanosecond timestamp or none
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, TimeZone, Utc};
    /// # use serde_derive::Deserialize;
    /// use chrono::serde::ts_nanoseconds_option::deserialize as from_nano_tsopt;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_nano_tsopt")]
    ///     time: Option<DateTime<Utc>>,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1526522699918355733 }"#)?;
    /// assert_eq!(my_s, S { time: Utc.timestamp_opt(1526522699, 918355733).single() });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionNanoSecondsTimestampVisitor)
    }

    struct OptionNanoSecondsTimestampVisitor;

    impl<'de> de::Visitor<'de> for OptionNanoSecondsTimestampVisitor {
        type Value = Option<DateTime<Utc>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in nanoseconds or none")
        }

        /// Deserialize a timestamp in nanoseconds since the epoch
        fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            d.deserialize_i64(NanoSecondsTimestampVisitor).map(Some)
        }

        /// Deserialize a timestamp in nanoseconds since the epoch
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        /// Deserialize a timestamp in nanoseconds since the epoch
        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }
}

/// Ser/de to/from timestamps in microseconds
///
/// Intended for use with `serde`'s `with` attribute.
///
/// # Example:
///
/// ```rust
/// # use chrono::{DateTime, Utc, NaiveDate};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::serde::ts_microseconds;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_microseconds")]
///     time: DateTime<Utc>,
/// }
///
/// let time = NaiveDate::from_ymd_opt(2018, 5, 17)
///     .unwrap()
///     .and_hms_micro_opt(02, 04, 59, 918355)
///     .unwrap()
///     .and_utc();
/// let my_s = S { time: time.clone() };
///
/// let as_string = serde_json::to_string(&my_s)?;
/// assert_eq!(as_string, r#"{"time":1526522699918355}"#);
/// let my_s: S = serde_json::from_str(&as_string)?;
/// assert_eq!(my_s.time, time);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub mod ts_microseconds {
    use core::fmt;
    use serde::{de, ser};

    use crate::serde::invalid_ts;
    use crate::{DateTime, Utc};

    use super::MicroSecondsTimestampVisitor;

    /// Serialize a UTC datetime into an integer number of microseconds since the epoch
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, Utc, NaiveDate};
    /// # use serde_derive::Serialize;
    /// use chrono::serde::ts_microseconds::serialize as to_micro_ts;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_micro_ts")]
    ///     time: DateTime<Utc>,
    /// }
    ///
    /// let my_s = S {
    ///     time: NaiveDate::from_ymd_opt(2018, 5, 17)
    ///         .unwrap()
    ///         .and_hms_micro_opt(02, 04, 59, 918355)
    ///         .unwrap()
    ///         .and_utc(),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918355}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.timestamp_micros())
    }

    /// Deserialize a `DateTime` from a microsecond timestamp
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, TimeZone, Utc};
    /// # use serde_derive::Deserialize;
    /// use chrono::serde::ts_microseconds::deserialize as from_micro_ts;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_micro_ts")]
    ///     time: DateTime<Utc>,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1526522699918355 }"#)?;
    /// assert_eq!(my_s, S { time: Utc.timestamp_opt(1526522699, 918355000).unwrap() });
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": -1 }"#)?;
    /// assert_eq!(my_s, S { time: Utc.timestamp_opt(-1, 999_999_000).unwrap() });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(MicroSecondsTimestampVisitor)
    }

    impl de::Visitor<'_> for MicroSecondsTimestampVisitor {
        type Value = DateTime<Utc>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in microseconds")
        }

        /// Deserialize a timestamp in milliseconds since the epoch
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_micros(value).ok_or_else(|| invalid_ts(value))
        }

        /// Deserialize a timestamp in milliseconds since the epoch
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp(
                (value / 1_000_000) as i64,
                ((value % 1_000_000) * 1_000) as u32,
            )
            .ok_or_else(|| invalid_ts(value))
        }
    }
}

/// Ser/de to/from optional timestamps in microseconds
///
/// Intended for use with `serde`'s `with` attribute.
///
/// # Example:
///
/// ```rust
/// # use chrono::{DateTime, Utc, NaiveDate};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::serde::ts_microseconds_option;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_microseconds_option")]
///     time: Option<DateTime<Utc>>,
/// }
///
/// let time = Some(
///     NaiveDate::from_ymd_opt(2018, 5, 17)
///         .unwrap()
///         .and_hms_micro_opt(02, 04, 59, 918355)
///         .unwrap()
///         .and_utc(),
/// );
/// let my_s = S { time: time.clone() };
///
/// let as_string = serde_json::to_string(&my_s)?;
/// assert_eq!(as_string, r#"{"time":1526522699918355}"#);
/// let my_s: S = serde_json::from_str(&as_string)?;
/// assert_eq!(my_s.time, time);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub mod ts_microseconds_option {
    use core::fmt;
    use serde::{de, ser};

    use super::MicroSecondsTimestampVisitor;
    use crate::{DateTime, Utc};

    /// Serialize a UTC datetime into an integer number of microseconds since the epoch or none
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, Utc, NaiveDate};
    /// # use serde_derive::Serialize;
    /// use chrono::serde::ts_microseconds_option::serialize as to_micro_tsopt;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_micro_tsopt")]
    ///     time: Option<DateTime<Utc>>,
    /// }
    ///
    /// let my_s = S {
    ///     time: Some(
    ///         NaiveDate::from_ymd_opt(2018, 5, 17)
    ///             .unwrap()
    ///             .and_hms_micro_opt(02, 04, 59, 918355)
    ///             .unwrap()
    ///             .and_utc(),
    ///     ),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918355}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(opt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.timestamp_micros()),
            None => serializer.serialize_none(),
        }
    }

    /// Deserialize a `DateTime` from a microsecond timestamp or none
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, TimeZone, Utc};
    /// # use serde_derive::Deserialize;
    /// use chrono::serde::ts_microseconds_option::deserialize as from_micro_tsopt;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_micro_tsopt")]
    ///     time: Option<DateTime<Utc>>,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1526522699918355 }"#)?;
    /// assert_eq!(my_s, S { time: Utc.timestamp_opt(1526522699, 918355000).single() });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionMicroSecondsTimestampVisitor)
    }

    struct OptionMicroSecondsTimestampVisitor;

    impl<'de> de::Visitor<'de> for OptionMicroSecondsTimestampVisitor {
        type Value = Option<DateTime<Utc>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in microseconds or none")
        }

        /// Deserialize a timestamp in microseconds since the epoch
        fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            d.deserialize_i64(MicroSecondsTimestampVisitor).map(Some)
        }

        /// Deserialize a timestamp in microseconds since the epoch
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        /// Deserialize a timestamp in microseconds since the epoch
        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }
}

/// Ser/de to/from timestamps in milliseconds
///
/// Intended for use with `serde`s `with` attribute.
///
/// # Example
///
/// ```rust
/// # use chrono::{DateTime, Utc, NaiveDate};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::serde::ts_milliseconds;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_milliseconds")]
///     time: DateTime<Utc>,
/// }
///
/// let time = NaiveDate::from_ymd_opt(2018, 5, 17)
///     .unwrap()
///     .and_hms_milli_opt(02, 04, 59, 918)
///     .unwrap()
///     .and_utc();
/// let my_s = S { time: time.clone() };
///
/// let as_string = serde_json::to_string(&my_s)?;
/// assert_eq!(as_string, r#"{"time":1526522699918}"#);
/// let my_s: S = serde_json::from_str(&as_string)?;
/// assert_eq!(my_s.time, time);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub mod ts_milliseconds {
    use core::fmt;
    use serde::{de, ser};

    use crate::serde::invalid_ts;
    use crate::{DateTime, Utc};

    use super::MilliSecondsTimestampVisitor;

    /// Serialize a UTC datetime into an integer number of milliseconds since the epoch
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, Utc, NaiveDate};
    /// # use serde_derive::Serialize;
    /// use chrono::serde::ts_milliseconds::serialize as to_milli_ts;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_milli_ts")]
    ///     time: DateTime<Utc>,
    /// }
    ///
    /// let my_s = S {
    ///     time: NaiveDate::from_ymd_opt(2018, 5, 17)
    ///         .unwrap()
    ///         .and_hms_milli_opt(02, 04, 59, 918)
    ///         .unwrap()
    ///         .and_utc(),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.timestamp_millis())
    }

    /// Deserialize a `DateTime` from a millisecond timestamp
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, TimeZone, Utc};
    /// # use serde_derive::Deserialize;
    /// use chrono::serde::ts_milliseconds::deserialize as from_milli_ts;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_milli_ts")]
    ///     time: DateTime<Utc>,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1526522699918 }"#)?;
    /// assert_eq!(my_s, S { time: Utc.timestamp_opt(1526522699, 918000000).unwrap() });
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": -1 }"#)?;
    /// assert_eq!(my_s, S { time: Utc.timestamp_opt(-1, 999_000_000).unwrap() });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(MilliSecondsTimestampVisitor).map(|dt| dt.with_timezone(&Utc))
    }

    impl de::Visitor<'_> for MilliSecondsTimestampVisitor {
        type Value = DateTime<Utc>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in milliseconds")
        }

        /// Deserialize a timestamp in milliseconds since the epoch
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_millis(value).ok_or_else(|| invalid_ts(value))
        }

        /// Deserialize a timestamp in milliseconds since the epoch
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp((value / 1000) as i64, ((value % 1000) * 1_000_000) as u32)
                .ok_or_else(|| invalid_ts(value))
        }
    }
}

/// Ser/de to/from optional timestamps in milliseconds
///
/// Intended for use with `serde`s `with` attribute.
///
/// # Example
///
/// ```rust
/// # use chrono::{DateTime, Utc, NaiveDate};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::serde::ts_milliseconds_option;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_milliseconds_option")]
///     time: Option<DateTime<Utc>>,
/// }
///
/// let time = Some(
///     NaiveDate::from_ymd_opt(2018, 5, 17)
///         .unwrap()
///         .and_hms_milli_opt(02, 04, 59, 918)
///         .unwrap()
///         .and_utc(),
/// );
/// let my_s = S { time: time.clone() };
///
/// let as_string = serde_json::to_string(&my_s)?;
/// assert_eq!(as_string, r#"{"time":1526522699918}"#);
/// let my_s: S = serde_json::from_str(&as_string)?;
/// assert_eq!(my_s.time, time);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub mod ts_milliseconds_option {
    use core::fmt;
    use serde::{de, ser};

    use super::MilliSecondsTimestampVisitor;
    use crate::{DateTime, Utc};

    /// Serialize a UTC datetime into an integer number of milliseconds since the epoch or none
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, Utc, NaiveDate};
    /// # use serde_derive::Serialize;
    /// use chrono::serde::ts_milliseconds_option::serialize as to_milli_tsopt;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_milli_tsopt")]
    ///     time: Option<DateTime<Utc>>,
    /// }
    ///
    /// let my_s = S {
    ///     time: Some(
    ///         NaiveDate::from_ymd_opt(2018, 5, 17)
    ///             .unwrap()
    ///             .and_hms_milli_opt(02, 04, 59, 918)
    ///             .unwrap()
    ///             .and_utc(),
    ///     ),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(opt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.timestamp_millis()),
            None => serializer.serialize_none(),
        }
    }

    /// Deserialize a `DateTime` from a millisecond timestamp or none
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{TimeZone, DateTime, Utc};
    /// # use serde_derive::Deserialize;
    /// use chrono::serde::ts_milliseconds_option::deserialize as from_milli_tsopt;
    ///
    /// #[derive(Deserialize, PartialEq, Debug)]
    /// #[serde(untagged)]
    /// enum E<T> {
    ///     V(T),
    /// }
    ///
    /// #[derive(Deserialize, PartialEq, Debug)]
    /// struct S {
    ///     #[serde(default, deserialize_with = "from_milli_tsopt")]
    ///     time: Option<DateTime<Utc>>,
    /// }
    ///
    /// let my_s: E<S> = serde_json::from_str(r#"{ "time": 1526522699918 }"#)?;
    /// assert_eq!(my_s, E::V(S { time: Some(Utc.timestamp_opt(1526522699, 918000000).unwrap()) }));
    /// let s: E<S> = serde_json::from_str(r#"{ "time": null }"#)?;
    /// assert_eq!(s, E::V(S { time: None }));
    /// let t: E<S> = serde_json::from_str(r#"{}"#)?;
    /// assert_eq!(t, E::V(S { time: None }));
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionMilliSecondsTimestampVisitor)
            .map(|opt| opt.map(|dt| dt.with_timezone(&Utc)))
    }

    struct OptionMilliSecondsTimestampVisitor;

    impl<'de> de::Visitor<'de> for OptionMilliSecondsTimestampVisitor {
        type Value = Option<DateTime<Utc>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in milliseconds or none")
        }

        /// Deserialize a timestamp in milliseconds since the epoch
        fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            d.deserialize_i64(MilliSecondsTimestampVisitor).map(Some)
        }

        /// Deserialize a timestamp in milliseconds since the epoch
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        /// Deserialize a timestamp in milliseconds since the epoch
        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }
}

/// Ser/de to/from timestamps in seconds
///
/// Intended for use with `serde`'s `with` attribute.
///
/// # Example:
///
/// ```rust
/// # use chrono::{TimeZone, DateTime, Utc};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::serde::ts_seconds;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_seconds")]
///     time: DateTime<Utc>,
/// }
///
/// let time = Utc.with_ymd_and_hms(2015, 5, 15, 10, 0, 0).unwrap();
/// let my_s = S { time: time.clone() };
///
/// let as_string = serde_json::to_string(&my_s)?;
/// assert_eq!(as_string, r#"{"time":1431684000}"#);
/// let my_s: S = serde_json::from_str(&as_string)?;
/// assert_eq!(my_s.time, time);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub mod ts_seconds {
    use core::fmt;
    use serde::{de, ser};

    use crate::serde::invalid_ts;
    use crate::{DateTime, Utc};

    use super::SecondsTimestampVisitor;

    /// Serialize a UTC datetime into an integer number of seconds since the epoch
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{TimeZone, DateTime, Utc};
    /// # use serde_derive::Serialize;
    /// use chrono::serde::ts_seconds::serialize as to_ts;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_ts")]
    ///     time: DateTime<Utc>,
    /// }
    ///
    /// let my_s = S { time: Utc.with_ymd_and_hms(2015, 5, 15, 10, 0, 0).unwrap() };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1431684000}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.timestamp())
    }

    /// Deserialize a `DateTime` from a seconds timestamp
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, TimeZone, Utc};
    /// # use serde_derive::Deserialize;
    /// use chrono::serde::ts_seconds::deserialize as from_ts;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_ts")]
    ///     time: DateTime<Utc>,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1431684000 }"#)?;
    /// assert_eq!(my_s, S { time: Utc.timestamp_opt(1431684000, 0).unwrap() });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(SecondsTimestampVisitor)
    }

    impl de::Visitor<'_> for SecondsTimestampVisitor {
        type Value = DateTime<Utc>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in seconds")
        }

        /// Deserialize a timestamp in seconds since the epoch
        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_secs(value).ok_or_else(|| invalid_ts(value))
        }

        /// Deserialize a timestamp in seconds since the epoch
        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value > i64::MAX as u64 {
                Err(invalid_ts(value))
            } else {
                DateTime::from_timestamp_secs(value as i64).ok_or_else(|| invalid_ts(value))
            }
        }
    }
}

/// Ser/de to/from optional timestamps in seconds
///
/// Intended for use with `serde`'s `with` attribute.
///
/// # Example:
///
/// ```rust
/// # use chrono::{TimeZone, DateTime, Utc};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::serde::ts_seconds_option;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_seconds_option")]
///     time: Option<DateTime<Utc>>,
/// }
///
/// let time = Some(Utc.with_ymd_and_hms(2015, 5, 15, 10, 0, 0).unwrap());
/// let my_s = S { time: time.clone() };
///
/// let as_string = serde_json::to_string(&my_s)?;
/// assert_eq!(as_string, r#"{"time":1431684000}"#);
/// let my_s: S = serde_json::from_str(&as_string)?;
/// assert_eq!(my_s.time, time);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub mod ts_seconds_option {
    use core::fmt;
    use serde::{de, ser};

    use super::SecondsTimestampVisitor;
    use crate::{DateTime, Utc};

    /// Serialize a UTC datetime into an integer number of seconds since the epoch or none
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{TimeZone, DateTime, Utc};
    /// # use serde_derive::Serialize;
    /// use chrono::serde::ts_seconds_option::serialize as to_tsopt;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_tsopt")]
    ///     time: Option<DateTime<Utc>>,
    /// }
    ///
    /// let my_s = S { time: Some(Utc.with_ymd_and_hms(2015, 5, 15, 10, 0, 0).unwrap()) };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1431684000}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(opt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.timestamp()),
            None => serializer.serialize_none(),
        }
    }

    /// Deserialize a `DateTime` from a seconds timestamp or none
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, TimeZone, Utc};
    /// # use serde_derive::Deserialize;
    /// use chrono::serde::ts_seconds_option::deserialize as from_tsopt;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_tsopt")]
    ///     time: Option<DateTime<Utc>>,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1431684000 }"#)?;
    /// assert_eq!(my_s, S { time: Utc.timestamp_opt(1431684000, 0).single() });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionSecondsTimestampVisitor)
    }

    struct OptionSecondsTimestampVisitor;

    impl<'de> de::Visitor<'de> for OptionSecondsTimestampVisitor {
        type Value = Option<DateTime<Utc>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp in seconds or none")
        }

        /// Deserialize a timestamp in seconds since the epoch
        fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            d.deserialize_i64(SecondsTimestampVisitor).map(Some)
        }

        /// Deserialize a timestamp in seconds since the epoch
        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        /// Deserialize a timestamp in seconds since the epoch
        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "clock")]
    use crate::Local;
    use crate::{DateTime, FixedOffset, TimeZone, Utc};
    use core::fmt;

    #[test]
    fn test_serde_serialize() {
        assert_eq!(
            serde_json::to_string(&Utc.with_ymd_and_hms(2014, 7, 24, 12, 34, 6).unwrap()).ok(),
            Some(r#""2014-07-24T12:34:06Z""#.to_owned())
        );
        assert_eq!(
            serde_json::to_string(
                &FixedOffset::east_opt(3660)
                    .unwrap()
                    .with_ymd_and_hms(2014, 7, 24, 12, 34, 6)
                    .unwrap()
            )
            .ok(),
            Some(r#""2014-07-24T12:34:06+01:01""#.to_owned())
        );
        assert_eq!(
            serde_json::to_string(
                &FixedOffset::east_opt(3650)
                    .unwrap()
                    .with_ymd_and_hms(2014, 7, 24, 12, 34, 6)
                    .unwrap()
            )
            .ok(),
            // An offset with seconds is not allowed by RFC 3339, so we round it to the nearest minute.
            // In this case `+01:00:50` becomes `+01:01`
            Some(r#""2014-07-24T12:34:06+01:01""#.to_owned())
        );
    }

    #[test]
    fn test_serde_deserialize() {
        // should check against the offset as well (the normal DateTime comparison will ignore them)
        fn norm<Tz: TimeZone>(dt: &Option<DateTime<Tz>>) -> Option<(&DateTime<Tz>, &Tz::Offset)> {
            dt.as_ref().map(|dt| (dt, dt.offset()))
        }

        let dt: Option<DateTime<Utc>> = serde_json::from_str(r#""2014-07-24T12:34:06Z""#).ok();
        assert_eq!(norm(&dt), norm(&Some(Utc.with_ymd_and_hms(2014, 7, 24, 12, 34, 6).unwrap())));
        let dt: Option<DateTime<Utc>> = serde_json::from_str(r#""2014-07-24T13:57:06+01:23""#).ok();
        assert_eq!(norm(&dt), norm(&Some(Utc.with_ymd_and_hms(2014, 7, 24, 12, 34, 6).unwrap())));

        let dt: Option<DateTime<FixedOffset>> =
            serde_json::from_str(r#""2014-07-24T12:34:06Z""#).ok();
        assert_eq!(
            norm(&dt),
            norm(&Some(
                FixedOffset::east_opt(0).unwrap().with_ymd_and_hms(2014, 7, 24, 12, 34, 6).unwrap()
            ))
        );
        let dt: Option<DateTime<FixedOffset>> =
            serde_json::from_str(r#""2014-07-24T13:57:06+01:23""#).ok();
        assert_eq!(
            norm(&dt),
            norm(&Some(
                FixedOffset::east_opt(60 * 60 + 23 * 60)
                    .unwrap()
                    .with_ymd_and_hms(2014, 7, 24, 13, 57, 6)
                    .unwrap()
            ))
        );

        // we don't know the exact local offset but we can check that
        // the conversion didn't change the instant itself
        #[cfg(feature = "clock")]
        {
            let dt: DateTime<Local> =
                serde_json::from_str(r#""2014-07-24T12:34:06Z""#).expect("local should parse");
            assert_eq!(dt, Utc.with_ymd_and_hms(2014, 7, 24, 12, 34, 6).unwrap());

            let dt: DateTime<Local> = serde_json::from_str(r#""2014-07-24T13:57:06+01:23""#)
                .expect("local should parse with offset");
            assert_eq!(dt, Utc.with_ymd_and_hms(2014, 7, 24, 12, 34, 6).unwrap());
        }

        assert!(serde_json::from_str::<DateTime<Utc>>(r#""2014-07-32T12:34:06Z""#).is_err());
        assert!(
            serde_json::from_str::<DateTime<FixedOffset>>(r#""2014-07-32T12:34:06Z""#).is_err()
        );
    }

    #[test]
    fn test_serde_bincode() {
        // Bincode is relevant to test separately from JSON because
        // it is not self-describing.
        use bincode::{deserialize, serialize};

        let dt = Utc.with_ymd_and_hms(2014, 7, 24, 12, 34, 6).unwrap();
        let encoded = serialize(&dt).unwrap();
        let decoded: DateTime<Utc> = deserialize(&encoded).unwrap();
        assert_eq!(dt, decoded);
        assert_eq!(dt.offset(), decoded.offset());
    }

    #[test]
    fn test_serde_no_offset_debug() {
        use crate::{MappedLocalTime, NaiveDate, NaiveDateTime, Offset};
        use core::fmt::Debug;

        #[derive(Clone)]
        struct TestTimeZone;
        impl Debug for TestTimeZone {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "TEST")
            }
        }
        impl TimeZone for TestTimeZone {
            type Offset = TestTimeZone;
            fn from_offset(_state: &TestTimeZone) -> TestTimeZone {
                TestTimeZone
            }
            fn offset_from_local_date(&self, _local: &NaiveDate) -> MappedLocalTime<TestTimeZone> {
                MappedLocalTime::Single(TestTimeZone)
            }
            fn offset_from_local_datetime(
                &self,
                _local: &NaiveDateTime,
            ) -> MappedLocalTime<TestTimeZone> {
                MappedLocalTime::Single(TestTimeZone)
            }
            fn offset_from_utc_date(&self, _utc: &NaiveDate) -> TestTimeZone {
                TestTimeZone
            }
            fn offset_from_utc_datetime(&self, _utc: &NaiveDateTime) -> TestTimeZone {
                TestTimeZone
            }
        }
        impl Offset for TestTimeZone {
            fn fix(&self) -> FixedOffset {
                FixedOffset::east_opt(15 * 60 * 60).unwrap()
            }
        }

        let tz = TestTimeZone;
        assert_eq!(format!("{:?}", &tz), "TEST");

        let dt = tz.with_ymd_and_hms(2023, 4, 24, 21, 10, 33).unwrap();
        let encoded = serde_json::to_string(&dt).unwrap();
        dbg!(&encoded);
        let decoded: DateTime<FixedOffset> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(dt, decoded);
        assert_eq!(dt.offset().fix(), *decoded.offset());
    }
}
