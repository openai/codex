use core::fmt;
use serde::{de, ser};

use super::NaiveDateTime;

/// Serialize a `NaiveDateTime` as an ISO 8601 string
///
/// See [the `naive::serde` module](crate::naive::serde) for alternate serialization formats.
impl ser::Serialize for NaiveDateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        struct FormatWrapped<'a, D: 'a> {
            inner: &'a D,
        }

        impl<D: fmt::Debug> fmt::Display for FormatWrapped<'_, D> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                self.inner.fmt(f)
            }
        }

        serializer.collect_str(&FormatWrapped { inner: &self })
    }
}

struct NaiveDateTimeVisitor;

impl de::Visitor<'_> for NaiveDateTimeVisitor {
    type Value = NaiveDateTime;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a formatted date and time string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

impl<'de> de::Deserialize<'de> for NaiveDateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_str(NaiveDateTimeVisitor)
    }
}

/// Used to serialize/deserialize from nanosecond-precision timestamps
///
/// # Example:
///
/// ```rust
/// # use chrono::{NaiveDate, NaiveDateTime};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::naive::serde::ts_nanoseconds;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_nanoseconds")]
///     time: NaiveDateTime,
/// }
///
/// let time = NaiveDate::from_ymd_opt(2018, 5, 17)
///     .unwrap()
///     .and_hms_nano_opt(02, 04, 59, 918355733)
///     .unwrap();
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
    use crate::{DateTime, NaiveDateTime};

    /// Serialize a datetime into an integer number of nanoseconds since the epoch
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
    /// # use chrono::{NaiveDate, NaiveDateTime};
    /// # use serde_derive::Serialize;
    /// use chrono::naive::serde::ts_nanoseconds::serialize as to_nano_ts;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_nano_ts")]
    ///     time: NaiveDateTime,
    /// }
    ///
    /// let my_s = S {
    ///     time: NaiveDate::from_ymd_opt(2018, 5, 17)
    ///         .unwrap()
    ///         .and_hms_nano_opt(02, 04, 59, 918355733)
    ///         .unwrap(),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918355733}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(dt: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.and_utc().timestamp_nanos_opt().ok_or(ser::Error::custom(
            "value out of range for a timestamp with nanosecond precision",
        ))?)
    }

    /// Deserialize a `NaiveDateTime` from a nanoseconds timestamp
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, NaiveDateTime};
    /// # use serde_derive::Deserialize;
    /// use chrono::naive::serde::ts_nanoseconds::deserialize as from_nano_ts;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_nano_ts")]
    ///     time: NaiveDateTime,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1526522699918355733 }"#)?;
    /// let expected = DateTime::from_timestamp(1526522699, 918355733).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: expected });
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": -1 }"#)?;
    /// let expected = DateTime::from_timestamp(-1, 999_999_999).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: expected });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<NaiveDateTime, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(NanoSecondsTimestampVisitor)
    }

    pub(super) struct NanoSecondsTimestampVisitor;

    impl de::Visitor<'_> for NanoSecondsTimestampVisitor {
        type Value = NaiveDateTime;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp(
                value.div_euclid(1_000_000_000),
                (value.rem_euclid(1_000_000_000)) as u32,
            )
            .map(|dt| dt.naive_utc())
            .ok_or_else(|| invalid_ts(value))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp((value / 1_000_000_000) as i64, (value % 1_000_000_000) as u32)
                .map(|dt| dt.naive_utc())
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
/// # use chrono::naive::{NaiveDate, NaiveDateTime};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::naive::serde::ts_nanoseconds_option;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_nanoseconds_option")]
///     time: Option<NaiveDateTime>,
/// }
///
/// let time = Some(
///     NaiveDate::from_ymd_opt(2018, 5, 17)
///         .unwrap()
///         .and_hms_nano_opt(02, 04, 59, 918355733)
///         .unwrap(),
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

    use super::ts_nanoseconds::NanoSecondsTimestampVisitor;
    use crate::NaiveDateTime;

    /// Serialize a datetime into an integer number of nanoseconds since the epoch or none
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
    /// # use chrono::naive::{NaiveDate, NaiveDateTime};
    /// # use serde_derive::Serialize;
    /// use chrono::naive::serde::ts_nanoseconds_option::serialize as to_nano_tsopt;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_nano_tsopt")]
    ///     time: Option<NaiveDateTime>,
    /// }
    ///
    /// let my_s = S {
    ///     time: Some(
    ///         NaiveDate::from_ymd_opt(2018, 5, 17)
    ///             .unwrap()
    ///             .and_hms_nano_opt(02, 04, 59, 918355733)
    ///             .unwrap(),
    ///     ),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918355733}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(opt: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.and_utc().timestamp_nanos_opt().ok_or(
                ser::Error::custom("value out of range for a timestamp with nanosecond precision"),
            )?),
            None => serializer.serialize_none(),
        }
    }

    /// Deserialize a `NaiveDateTime` from a nanosecond timestamp or none
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, NaiveDateTime};
    /// # use serde_derive::Deserialize;
    /// use chrono::naive::serde::ts_nanoseconds_option::deserialize as from_nano_tsopt;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_nano_tsopt")]
    ///     time: Option<NaiveDateTime>,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1526522699918355733 }"#)?;
    /// let expected = DateTime::from_timestamp(1526522699, 918355733).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: Some(expected) });
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": -1 }"#)?;
    /// let expected = DateTime::from_timestamp(-1, 999_999_999).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: Some(expected) });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionNanoSecondsTimestampVisitor)
    }

    struct OptionNanoSecondsTimestampVisitor;

    impl<'de> de::Visitor<'de> for OptionNanoSecondsTimestampVisitor {
        type Value = Option<NaiveDateTime>;

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

/// Used to serialize/deserialize from microsecond-precision timestamps
///
/// # Example:
///
/// ```rust
/// # use chrono::{NaiveDate, NaiveDateTime};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::naive::serde::ts_microseconds;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_microseconds")]
///     time: NaiveDateTime,
/// }
///
/// let time = NaiveDate::from_ymd_opt(2018, 5, 17)
///     .unwrap()
///     .and_hms_micro_opt(02, 04, 59, 918355)
///     .unwrap();
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
    use crate::{DateTime, NaiveDateTime};

    /// Serialize a datetime into an integer number of microseconds since the epoch
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{NaiveDate, NaiveDateTime};
    /// # use serde_derive::Serialize;
    /// use chrono::naive::serde::ts_microseconds::serialize as to_micro_ts;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_micro_ts")]
    ///     time: NaiveDateTime,
    /// }
    ///
    /// let my_s = S {
    ///     time: NaiveDate::from_ymd_opt(2018, 5, 17)
    ///         .unwrap()
    ///         .and_hms_micro_opt(02, 04, 59, 918355)
    ///         .unwrap(),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918355}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(dt: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.and_utc().timestamp_micros())
    }

    /// Deserialize a `NaiveDateTime` from a microseconds timestamp
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, NaiveDateTime};
    /// # use serde_derive::Deserialize;
    /// use chrono::naive::serde::ts_microseconds::deserialize as from_micro_ts;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_micro_ts")]
    ///     time: NaiveDateTime,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1526522699918355 }"#)?;
    /// let expected = DateTime::from_timestamp(1526522699, 918355000).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: expected });
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": -1 }"#)?;
    /// let expected = DateTime::from_timestamp(-1, 999_999_000).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: expected });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<NaiveDateTime, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(MicroSecondsTimestampVisitor)
    }

    pub(super) struct MicroSecondsTimestampVisitor;

    impl de::Visitor<'_> for MicroSecondsTimestampVisitor {
        type Value = NaiveDateTime;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_micros(value)
                .map(|dt| dt.naive_utc())
                .ok_or_else(|| invalid_ts(value))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp(
                (value / 1_000_000) as i64,
                ((value % 1_000_000) * 1_000) as u32,
            )
            .map(|dt| dt.naive_utc())
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
/// # use chrono::naive::{NaiveDate, NaiveDateTime};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::naive::serde::ts_microseconds_option;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_microseconds_option")]
///     time: Option<NaiveDateTime>,
/// }
///
/// let time = Some(
///     NaiveDate::from_ymd_opt(2018, 5, 17)
///         .unwrap()
///         .and_hms_micro_opt(02, 04, 59, 918355)
///         .unwrap(),
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

    use super::ts_microseconds::MicroSecondsTimestampVisitor;
    use crate::NaiveDateTime;

    /// Serialize a datetime into an integer number of microseconds since the epoch or none
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::naive::{NaiveDate, NaiveDateTime};
    /// # use serde_derive::Serialize;
    /// use chrono::naive::serde::ts_microseconds_option::serialize as to_micro_tsopt;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_micro_tsopt")]
    ///     time: Option<NaiveDateTime>,
    /// }
    ///
    /// let my_s = S {
    ///     time: Some(
    ///         NaiveDate::from_ymd_opt(2018, 5, 17)
    ///             .unwrap()
    ///             .and_hms_micro_opt(02, 04, 59, 918355)
    ///             .unwrap(),
    ///     ),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918355}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(opt: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.and_utc().timestamp_micros()),
            None => serializer.serialize_none(),
        }
    }

    /// Deserialize a `NaiveDateTime` from a nanosecond timestamp or none
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, NaiveDateTime};
    /// # use serde_derive::Deserialize;
    /// use chrono::naive::serde::ts_microseconds_option::deserialize as from_micro_tsopt;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_micro_tsopt")]
    ///     time: Option<NaiveDateTime>,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1526522699918355 }"#)?;
    /// let expected = DateTime::from_timestamp(1526522699, 918355000).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: Some(expected) });
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": -1 }"#)?;
    /// let expected = DateTime::from_timestamp(-1, 999_999_000).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: Some(expected) });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionMicroSecondsTimestampVisitor)
    }

    struct OptionMicroSecondsTimestampVisitor;

    impl<'de> de::Visitor<'de> for OptionMicroSecondsTimestampVisitor {
        type Value = Option<NaiveDateTime>;

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

/// Used to serialize/deserialize from millisecond-precision timestamps
///
/// # Example:
///
/// ```rust
/// # use chrono::{NaiveDate, NaiveDateTime};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::naive::serde::ts_milliseconds;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_milliseconds")]
///     time: NaiveDateTime,
/// }
///
/// let time =
///     NaiveDate::from_ymd_opt(2018, 5, 17).unwrap().and_hms_milli_opt(02, 04, 59, 918).unwrap();
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
    use crate::{DateTime, NaiveDateTime};

    /// Serialize a datetime into an integer number of milliseconds since the epoch
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{NaiveDate, NaiveDateTime};
    /// # use serde_derive::Serialize;
    /// use chrono::naive::serde::ts_milliseconds::serialize as to_milli_ts;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_milli_ts")]
    ///     time: NaiveDateTime,
    /// }
    ///
    /// let my_s = S {
    ///     time: NaiveDate::from_ymd_opt(2018, 5, 17)
    ///         .unwrap()
    ///         .and_hms_milli_opt(02, 04, 59, 918)
    ///         .unwrap(),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(dt: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.and_utc().timestamp_millis())
    }

    /// Deserialize a `NaiveDateTime` from a milliseconds timestamp
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, NaiveDateTime};
    /// # use serde_derive::Deserialize;
    /// use chrono::naive::serde::ts_milliseconds::deserialize as from_milli_ts;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_milli_ts")]
    ///     time: NaiveDateTime,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1526522699918 }"#)?;
    /// let expected = DateTime::from_timestamp(1526522699, 918000000).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: expected });
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": -1 }"#)?;
    /// let expected = DateTime::from_timestamp(-1, 999_000_000).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: expected });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<NaiveDateTime, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(MilliSecondsTimestampVisitor)
    }

    pub(super) struct MilliSecondsTimestampVisitor;

    impl de::Visitor<'_> for MilliSecondsTimestampVisitor {
        type Value = NaiveDateTime;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_millis(value)
                .map(|dt| dt.naive_utc())
                .ok_or_else(|| invalid_ts(value))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp((value / 1000) as i64, ((value % 1000) * 1_000_000) as u32)
                .map(|dt| dt.naive_utc())
                .ok_or_else(|| invalid_ts(value))
        }
    }
}

/// Ser/de to/from optional timestamps in milliseconds
///
/// Intended for use with `serde`'s `with` attribute.
///
/// # Example:
///
/// ```rust
/// # use chrono::naive::{NaiveDate, NaiveDateTime};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::naive::serde::ts_milliseconds_option;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_milliseconds_option")]
///     time: Option<NaiveDateTime>,
/// }
///
/// let time = Some(
///     NaiveDate::from_ymd_opt(2018, 5, 17).unwrap().and_hms_milli_opt(02, 04, 59, 918).unwrap(),
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

    use super::ts_milliseconds::MilliSecondsTimestampVisitor;
    use crate::NaiveDateTime;

    /// Serialize a datetime into an integer number of milliseconds since the epoch or none
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::naive::{NaiveDate, NaiveDateTime};
    /// # use serde_derive::Serialize;
    /// use chrono::naive::serde::ts_milliseconds_option::serialize as to_milli_tsopt;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_milli_tsopt")]
    ///     time: Option<NaiveDateTime>,
    /// }
    ///
    /// let my_s = S {
    ///     time: Some(
    ///         NaiveDate::from_ymd_opt(2018, 5, 17)
    ///             .unwrap()
    ///             .and_hms_milli_opt(02, 04, 59, 918)
    ///             .unwrap(),
    ///     ),
    /// };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699918}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(opt: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.and_utc().timestamp_millis()),
            None => serializer.serialize_none(),
        }
    }

    /// Deserialize a `NaiveDateTime` from a millisecond timestamp or none
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, NaiveDateTime};
    /// # use serde_derive::Deserialize;
    /// use chrono::naive::serde::ts_milliseconds_option::deserialize as from_milli_tsopt;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_milli_tsopt")]
    ///     time: Option<NaiveDateTime>,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1526522699918 }"#)?;
    /// let expected = DateTime::from_timestamp(1526522699, 918000000).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: Some(expected) });
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": -1 }"#)?;
    /// let expected = DateTime::from_timestamp(-1, 999_000_000).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: Some(expected) });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionMilliSecondsTimestampVisitor)
    }

    struct OptionMilliSecondsTimestampVisitor;

    impl<'de> de::Visitor<'de> for OptionMilliSecondsTimestampVisitor {
        type Value = Option<NaiveDateTime>;

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

/// Used to serialize/deserialize from second-precision timestamps
///
/// # Example:
///
/// ```rust
/// # use chrono::{NaiveDate, NaiveDateTime};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::naive::serde::ts_seconds;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_seconds")]
///     time: NaiveDateTime,
/// }
///
/// let time = NaiveDate::from_ymd_opt(2015, 5, 15).unwrap().and_hms_opt(10, 0, 0).unwrap();
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
    use crate::{DateTime, NaiveDateTime};

    /// Serialize a datetime into an integer number of seconds since the epoch
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{NaiveDate, NaiveDateTime};
    /// # use serde_derive::Serialize;
    /// use chrono::naive::serde::ts_seconds::serialize as to_ts;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_ts")]
    ///     time: NaiveDateTime,
    /// }
    ///
    /// let my_s =
    ///     S { time: NaiveDate::from_ymd_opt(2015, 5, 15).unwrap().and_hms_opt(10, 0, 0).unwrap() };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1431684000}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(dt: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_i64(dt.and_utc().timestamp())
    }

    /// Deserialize a `NaiveDateTime` from a seconds timestamp
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, NaiveDateTime};
    /// # use serde_derive::Deserialize;
    /// use chrono::naive::serde::ts_seconds::deserialize as from_ts;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_ts")]
    ///     time: NaiveDateTime,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1431684000 }"#)?;
    /// let expected = DateTime::from_timestamp_secs(1431684000).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: expected });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<NaiveDateTime, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_i64(SecondsTimestampVisitor)
    }

    pub(super) struct SecondsTimestampVisitor;

    impl de::Visitor<'_> for SecondsTimestampVisitor {
        type Value = NaiveDateTime;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a unix timestamp")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            DateTime::from_timestamp_secs(value)
                .map(|dt| dt.naive_utc())
                .ok_or_else(|| invalid_ts(value))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value > i64::MAX as u64 {
                Err(invalid_ts(value))
            } else {
                DateTime::from_timestamp_secs(value as i64)
                    .map(|dt| dt.naive_utc())
                    .ok_or_else(|| invalid_ts(value))
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
/// # use chrono::naive::{NaiveDate, NaiveDateTime};
/// # use serde_derive::{Deserialize, Serialize};
/// use chrono::naive::serde::ts_seconds_option;
/// #[derive(Deserialize, Serialize)]
/// struct S {
///     #[serde(with = "ts_seconds_option")]
///     time: Option<NaiveDateTime>,
/// }
///
/// let time = Some(NaiveDate::from_ymd_opt(2018, 5, 17).unwrap().and_hms_opt(02, 04, 59).unwrap());
/// let my_s = S { time: time.clone() };
///
/// let as_string = serde_json::to_string(&my_s)?;
/// assert_eq!(as_string, r#"{"time":1526522699}"#);
/// let my_s: S = serde_json::from_str(&as_string)?;
/// assert_eq!(my_s.time, time);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub mod ts_seconds_option {
    use core::fmt;
    use serde::{de, ser};

    use super::ts_seconds::SecondsTimestampVisitor;
    use crate::NaiveDateTime;

    /// Serialize a datetime into an integer number of seconds since the epoch or none
    ///
    /// Intended for use with `serde`s `serialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::naive::{NaiveDate, NaiveDateTime};
    /// # use serde_derive::Serialize;
    /// use chrono::naive::serde::ts_seconds_option::serialize as to_tsopt;
    /// #[derive(Serialize)]
    /// struct S {
    ///     #[serde(serialize_with = "to_tsopt")]
    ///     time: Option<NaiveDateTime>,
    /// }
    ///
    /// let expected = NaiveDate::from_ymd_opt(2018, 5, 17).unwrap().and_hms_opt(02, 04, 59).unwrap();
    /// let my_s = S { time: Some(expected) };
    /// let as_string = serde_json::to_string(&my_s)?;
    /// assert_eq!(as_string, r#"{"time":1526522699}"#);
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn serialize<S>(opt: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *opt {
            Some(ref dt) => serializer.serialize_some(&dt.and_utc().timestamp()),
            None => serializer.serialize_none(),
        }
    }

    /// Deserialize a `NaiveDateTime` from a second timestamp or none
    ///
    /// Intended for use with `serde`s `deserialize_with` attribute.
    ///
    /// # Example:
    ///
    /// ```rust
    /// # use chrono::{DateTime, NaiveDateTime};
    /// # use serde_derive::Deserialize;
    /// use chrono::naive::serde::ts_seconds_option::deserialize as from_tsopt;
    /// #[derive(Debug, PartialEq, Deserialize)]
    /// struct S {
    ///     #[serde(deserialize_with = "from_tsopt")]
    ///     time: Option<NaiveDateTime>,
    /// }
    ///
    /// let my_s: S = serde_json::from_str(r#"{ "time": 1431684000 }"#)?;
    /// let expected = DateTime::from_timestamp_secs(1431684000).unwrap().naive_utc();
    /// assert_eq!(my_s, S { time: Some(expected) });
    /// # Ok::<(), serde_json::Error>(())
    /// ```
    pub fn deserialize<'de, D>(d: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_option(OptionSecondsTimestampVisitor)
    }

    struct OptionSecondsTimestampVisitor;

    impl<'de> de::Visitor<'de> for OptionSecondsTimestampVisitor {
        type Value = Option<NaiveDateTime>;

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
    use crate::serde::ts_nanoseconds_option;
    use crate::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};

    use bincode::{deserialize, serialize};
    use serde_derive::{Deserialize, Serialize};

    #[test]
    fn test_serde_serialize() {
        assert_eq!(
            serde_json::to_string(
                &NaiveDate::from_ymd_opt(2016, 7, 8)
                    .unwrap()
                    .and_hms_milli_opt(9, 10, 48, 90)
                    .unwrap()
            )
            .ok(),
            Some(r#""2016-07-08T09:10:48.090""#.into())
        );
        assert_eq!(
            serde_json::to_string(
                &NaiveDate::from_ymd_opt(2014, 7, 24).unwrap().and_hms_opt(12, 34, 6).unwrap()
            )
            .ok(),
            Some(r#""2014-07-24T12:34:06""#.into())
        );
        assert_eq!(
            serde_json::to_string(
                &NaiveDate::from_ymd_opt(0, 1, 1)
                    .unwrap()
                    .and_hms_milli_opt(0, 0, 59, 1_000)
                    .unwrap()
            )
            .ok(),
            Some(r#""0000-01-01T00:00:60""#.into())
        );
        assert_eq!(
            serde_json::to_string(
                &NaiveDate::from_ymd_opt(-1, 12, 31)
                    .unwrap()
                    .and_hms_nano_opt(23, 59, 59, 7)
                    .unwrap()
            )
            .ok(),
            Some(r#""-0001-12-31T23:59:59.000000007""#.into())
        );
        assert_eq!(
            serde_json::to_string(&NaiveDate::MIN.and_hms_opt(0, 0, 0).unwrap()).ok(),
            Some(r#""-262143-01-01T00:00:00""#.into())
        );
        assert_eq!(
            serde_json::to_string(
                &NaiveDate::MAX.and_hms_nano_opt(23, 59, 59, 1_999_999_999).unwrap()
            )
            .ok(),
            Some(r#""+262142-12-31T23:59:60.999999999""#.into())
        );
    }

    #[test]
    fn test_serde_deserialize() {
        let from_str = serde_json::from_str::<NaiveDateTime>;

        assert_eq!(
            from_str(r#""2016-07-08T09:10:48.090""#).ok(),
            Some(
                NaiveDate::from_ymd_opt(2016, 7, 8)
                    .unwrap()
                    .and_hms_milli_opt(9, 10, 48, 90)
                    .unwrap()
            )
        );
        assert_eq!(
            from_str(r#""2016-7-8T9:10:48.09""#).ok(),
            Some(
                NaiveDate::from_ymd_opt(2016, 7, 8)
                    .unwrap()
                    .and_hms_milli_opt(9, 10, 48, 90)
                    .unwrap()
            )
        );
        assert_eq!(
            from_str(r#""2014-07-24T12:34:06""#).ok(),
            Some(NaiveDate::from_ymd_opt(2014, 7, 24).unwrap().and_hms_opt(12, 34, 6).unwrap())
        );
        assert_eq!(
            from_str(r#""0000-01-01T00:00:60""#).ok(),
            Some(
                NaiveDate::from_ymd_opt(0, 1, 1)
                    .unwrap()
                    .and_hms_milli_opt(0, 0, 59, 1_000)
                    .unwrap()
            )
        );
        assert_eq!(
            from_str(r#""0-1-1T0:0:60""#).ok(),
            Some(
                NaiveDate::from_ymd_opt(0, 1, 1)
                    .unwrap()
                    .and_hms_milli_opt(0, 0, 59, 1_000)
                    .unwrap()
            )
        );
        assert_eq!(
            from_str(r#""-0001-12-31T23:59:59.000000007""#).ok(),
            Some(
                NaiveDate::from_ymd_opt(-1, 12, 31)
                    .unwrap()
                    .and_hms_nano_opt(23, 59, 59, 7)
                    .unwrap()
            )
        );
        assert_eq!(
            from_str(r#""-262143-01-01T00:00:00""#).ok(),
            Some(NaiveDate::MIN.and_hms_opt(0, 0, 0).unwrap())
        );
        assert_eq!(
            from_str(r#""+262142-12-31T23:59:60.999999999""#).ok(),
            Some(NaiveDate::MAX.and_hms_nano_opt(23, 59, 59, 1_999_999_999).unwrap())
        );
        assert_eq!(
            from_str(r#""+262142-12-31T23:59:60.9999999999997""#).ok(), // excess digits are ignored
            Some(NaiveDate::MAX.and_hms_nano_opt(23, 59, 59, 1_999_999_999).unwrap())
        );

        // bad formats
        assert!(from_str(r#""""#).is_err());
        assert!(from_str(r#""2016-07-08""#).is_err());
        assert!(from_str(r#""09:10:48.090""#).is_err());
        assert!(from_str(r#""20160708T091048.090""#).is_err());
        assert!(from_str(r#""2000-00-00T00:00:00""#).is_err());
        assert!(from_str(r#""2000-02-30T00:00:00""#).is_err());
        assert!(from_str(r#""2001-02-29T00:00:00""#).is_err());
        assert!(from_str(r#""2002-02-28T24:00:00""#).is_err());
        assert!(from_str(r#""2002-02-28T23:60:00""#).is_err());
        assert!(from_str(r#""2002-02-28T23:59:61""#).is_err());
        assert!(from_str(r#""2016-07-08T09:10:48,090""#).is_err());
        assert!(from_str(r#""2016-07-08 09:10:48.090""#).is_err());
        assert!(from_str(r#""2016-007-08T09:10:48.090""#).is_err());
        assert!(from_str(r#""yyyy-mm-ddThh:mm:ss.fffffffff""#).is_err());
        assert!(from_str(r#"20160708000000"#).is_err());
        assert!(from_str(r#"{}"#).is_err());
        // pre-0.3.0 rustc-serialize format is now invalid
        assert!(from_str(r#"{"date":{"ymdf":20},"time":{"secs":0,"frac":0}}"#).is_err());
        assert!(from_str(r#"null"#).is_err());
    }

    // Bincode is relevant to test separately from JSON because
    // it is not self-describing.
    #[test]
    fn test_serde_bincode() {
        let dt =
            NaiveDate::from_ymd_opt(2016, 7, 8).unwrap().and_hms_milli_opt(9, 10, 48, 90).unwrap();
        let encoded = serialize(&dt).unwrap();
        let decoded: NaiveDateTime = deserialize(&encoded).unwrap();
        assert_eq!(dt, decoded);
    }

    #[test]
    fn test_serde_bincode_optional() {
        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct Test {
            one: Option<i64>,
            #[serde(with = "ts_nanoseconds_option")]
            two: Option<DateTime<Utc>>,
        }

        let expected =
            Test { one: Some(1), two: Some(Utc.with_ymd_and_hms(1970, 1, 1, 0, 1, 1).unwrap()) };
        let bytes: Vec<u8> = serialize(&expected).unwrap();
        let actual = deserialize::<Test>(&(bytes)).unwrap();

        assert_eq!(expected, actual);
    }
}
