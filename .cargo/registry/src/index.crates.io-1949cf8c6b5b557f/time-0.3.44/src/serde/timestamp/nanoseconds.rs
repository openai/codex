//! Treat an [`OffsetDateTime`] as a [Unix timestamp] with nanoseconds for
//! the purposes of serde.
//!
//! Use this module in combination with serde's [`#[with]`][with] attribute.
//!
//! When deserializing, the offset is assumed to be UTC.
//!
//! [Unix timestamp]: https://en.wikipedia.org/wiki/Unix_time
//! [with]: https://serde.rs/field-attrs.html#with

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::OffsetDateTime;

/// Serialize an `OffsetDateTime` as its Unix timestamp with nanoseconds
#[inline]
pub fn serialize<S: Serializer>(
    datetime: &OffsetDateTime,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    datetime.unix_timestamp_nanos().serialize(serializer)
}

/// Deserialize an `OffsetDateTime` from its Unix timestamp with nanoseconds
#[inline]
pub fn deserialize<'a, D: Deserializer<'a>>(deserializer: D) -> Result<OffsetDateTime, D::Error> {
    OffsetDateTime::from_unix_timestamp_nanos(<_>::deserialize(deserializer)?)
        .map_err(|err| de::Error::invalid_value(de::Unexpected::Signed(err.value), &err))
}

/// Treat an `Option<OffsetDateTime>` as a [Unix timestamp] with nanoseconds
/// for the purposes of serde.
///
/// Use this module in combination with serde's [`#[with]`][with] attribute.
///
/// When deserializing, the offset is assumed to be UTC.
///
/// [Unix timestamp]: https://en.wikipedia.org/wiki/Unix_time
/// [with]: https://serde.rs/field-attrs.html#with
pub mod option {
    use super::*;

    /// Serialize an `Option<OffsetDateTime>` as its Unix timestamp with nanoseconds
    #[inline]
    pub fn serialize<S: Serializer>(
        option: &Option<OffsetDateTime>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        option
            .map(OffsetDateTime::unix_timestamp_nanos)
            .serialize(serializer)
    }

    /// Deserialize an `Option<OffsetDateTime>` from its Unix timestamp with nanoseconds
    #[inline]
    pub fn deserialize<'a, D: Deserializer<'a>>(
        deserializer: D,
    ) -> Result<Option<OffsetDateTime>, D::Error> {
        Option::deserialize(deserializer)?
            .map(OffsetDateTime::from_unix_timestamp_nanos)
            .transpose()
            .map_err(|err| de::Error::invalid_value(de::Unexpected::Signed(err.value), &err))
    }
}
