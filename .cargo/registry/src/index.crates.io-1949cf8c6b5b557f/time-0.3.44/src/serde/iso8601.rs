//! Use the well-known [ISO 8601 format] when serializing and deserializing an [`OffsetDateTime`].
//!
//! Use this module in combination with serde's [`#[with]`][with] attribute.
//!
//! [ISO 8601 format]: https://www.iso.org/iso-8601-date-and-time-format.html
//! [with]: https://serde.rs/field-attrs.html#with

#[cfg(feature = "parsing")]
use core::marker::PhantomData;

#[cfg(feature = "formatting")]
use serde::ser::Error as _;
#[cfg(feature = "parsing")]
use serde::Deserializer;
#[cfg(feature = "formatting")]
use serde::{Serialize, Serializer};

#[cfg(feature = "parsing")]
use super::Visitor;
use crate::format_description::well_known::iso8601::{Config, EncodedConfig};
use crate::format_description::well_known::Iso8601;
use crate::OffsetDateTime;

/// The configuration of ISO 8601 used for serde implementations.
pub(crate) const SERDE_CONFIG: EncodedConfig =
    Config::DEFAULT.set_year_is_six_digits(true).encode();

/// Serialize an [`OffsetDateTime`] using the well-known ISO 8601 format.
#[cfg(feature = "formatting")]
#[inline]
pub fn serialize<S: Serializer>(
    datetime: &OffsetDateTime,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    datetime
        .format(&Iso8601::<SERDE_CONFIG>)
        .map_err(S::Error::custom)?
        .serialize(serializer)
}

/// Deserialize an [`OffsetDateTime`] from its ISO 8601 representation.
#[cfg(feature = "parsing")]
#[inline]
pub fn deserialize<'a, D: Deserializer<'a>>(deserializer: D) -> Result<OffsetDateTime, D::Error> {
    deserializer.deserialize_str(Visitor::<Iso8601<SERDE_CONFIG>>(PhantomData))
}

/// Use the well-known ISO 8601 format when serializing and deserializing an
/// [`Option<OffsetDateTime>`].
///
/// Use this module in combination with serde's [`#[with]`][with] attribute.
///
/// [ISO 8601 format]: https://www.iso.org/iso-8601-date-and-time-format.html
/// [with]: https://serde.rs/field-attrs.html#with
pub mod option {
    use super::*;

    /// Serialize an [`Option<OffsetDateTime>`] using the well-known ISO 8601 format.
    #[cfg(feature = "formatting")]
    #[inline]
    pub fn serialize<S: Serializer>(
        option: &Option<OffsetDateTime>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        option
            .map(|odt| odt.format(&Iso8601::<SERDE_CONFIG>))
            .transpose()
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }

    /// Deserialize an [`Option<OffsetDateTime>`] from its ISO 8601 representation.
    #[cfg(feature = "parsing")]
    #[inline]
    pub fn deserialize<'a, D: Deserializer<'a>>(
        deserializer: D,
    ) -> Result<Option<OffsetDateTime>, D::Error> {
        deserializer.deserialize_option(Visitor::<Option<Iso8601<SERDE_CONFIG>>>(PhantomData))
    }
}
