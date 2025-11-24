use std::convert::{TryFrom, TryInto};
use std::time::{Duration, SystemTime};

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Converts a `SystemTime` object into a float timestamp.
pub fn datetime_to_timestamp(st: &SystemTime) -> f64 {
    match st.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(duration) => duration.as_secs_f64(),
        Err(_) => 0.0,
    }
}

pub fn timestamp_to_datetime(ts: f64) -> Option<SystemTime> {
    let duration = Duration::try_from_secs_f64(ts).ok()?;
    SystemTime::UNIX_EPOCH.checked_add(duration)
}

pub fn to_rfc3339(st: &SystemTime) -> String {
    st.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|duration| TryFrom::try_from(duration).ok())
        .and_then(|duration| OffsetDateTime::UNIX_EPOCH.checked_add(duration))
        .and_then(|dt| dt.format(&Rfc3339).ok())
        .unwrap_or_default()
}

pub mod ts_seconds_float {
    use std::fmt;

    use serde::{de, ser};

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<SystemTime, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_any(SecondsTimestampVisitor)
    }

    pub fn serialize<S>(st: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match st.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => {
                if duration.subsec_nanos() == 0 {
                    serializer.serialize_u64(duration.as_secs())
                } else {
                    serializer.serialize_f64(duration.as_secs_f64())
                }
            }
            Err(_) => Err(ser::Error::custom(format!(
                "invalid `SystemTime` instance: {st:?}"
            ))),
        }
    }

    struct SecondsTimestampVisitor;

    impl<'de> de::Visitor<'de> for SecondsTimestampVisitor {
        type Value = SystemTime;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "a unix timestamp")
        }

        fn visit_f64<E>(self, value: f64) -> Result<SystemTime, E>
        where
            E: de::Error,
        {
            match timestamp_to_datetime(value) {
                Some(st) => Ok(st),
                None => Err(E::custom(format!("invalid timestamp: {value}"))),
            }
        }

        fn visit_i64<E>(self, value: i64) -> Result<SystemTime, E>
        where
            E: de::Error,
        {
            let value = value.try_into().map_err(|e| E::custom(format!("{e}")))?;
            let duration = Duration::from_secs(value);
            match SystemTime::UNIX_EPOCH.checked_add(duration) {
                Some(st) => Ok(st),
                None => Err(E::custom(format!("invalid timestamp: {value}"))),
            }
        }

        fn visit_u64<E>(self, value: u64) -> Result<SystemTime, E>
        where
            E: de::Error,
        {
            let duration = Duration::from_secs(value);
            match SystemTime::UNIX_EPOCH.checked_add(duration) {
                Some(st) => Ok(st),
                None => Err(E::custom(format!("invalid timestamp: {value}"))),
            }
        }

        fn visit_str<E>(self, value: &str) -> Result<SystemTime, E>
        where
            E: de::Error,
        {
            let rfc3339_deser = super::ts_rfc3339::Rfc3339Deserializer;
            rfc3339_deser.visit_str(value)
        }
    }
}

pub mod ts_rfc3339 {
    use std::fmt;

    use serde::{de, ser};

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<SystemTime, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        d.deserialize_any(Rfc3339Deserializer)
    }

    pub fn serialize<S>(st: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match st
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|duration| TryFrom::try_from(duration).ok())
            .and_then(|duration| OffsetDateTime::UNIX_EPOCH.checked_add(duration))
            .and_then(|dt| dt.format(&Rfc3339).ok())
        {
            Some(formatted) => serializer.serialize_str(&formatted),
            None => Err(ser::Error::custom(format!(
                "invalid `SystemTime` instance: {st:?}"
            ))),
        }
    }

    pub(super) struct Rfc3339Deserializer;

    impl<'de> de::Visitor<'de> for Rfc3339Deserializer {
        type Value = SystemTime;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "an RFC3339 timestamp")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let dt = OffsetDateTime::parse(v, &Rfc3339).map_err(|e| E::custom(format!("{e}")))?;
            let secs = u64::try_from(dt.unix_timestamp()).map_err(|e| E::custom(format!("{e}")))?;
            let nanos = dt.nanosecond();
            let duration = Duration::new(secs, nanos);
            SystemTime::UNIX_EPOCH
                .checked_add(duration)
                .ok_or_else(|| E::custom("invalid timestamp"))
        }
    }
}

pub mod ts_rfc3339_opt {
    use serde::{de, ser};

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Option<SystemTime>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        ts_rfc3339::deserialize(d).map(Some)
    }

    pub fn serialize<S>(st: &Option<SystemTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match st {
            Some(st) => ts_rfc3339::serialize(st, serializer),
            None => serializer.serialize_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::timestamp_to_datetime;

    #[test]
    fn test_timestamp_to_datetime() {
        assert!(timestamp_to_datetime(-10000.0).is_none());
        assert!(timestamp_to_datetime(f64::INFINITY).is_none());
        assert!(timestamp_to_datetime(f64::MAX).is_none());
        assert!(timestamp_to_datetime(123123123.0).is_some());
    }
}
