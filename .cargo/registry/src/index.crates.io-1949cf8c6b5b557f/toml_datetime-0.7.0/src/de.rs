//! Deserialzation support for [`Datetime`][crate::Datetime]

use alloc::string::ToString;

use serde::de::value::BorrowedStrDeserializer;
use serde::de::IntoDeserializer;

/// Check if deserializing a [`Datetime`][crate::Datetime]
pub fn is_datetime(name: &'static str) -> bool {
    crate::datetime::is_datetime(name)
}

/// Deserializer / format support for emitting [`Datetime`][crate::Datetime]
pub struct DatetimeDeserializer<E> {
    date: Option<crate::Datetime>,
    _error: core::marker::PhantomData<E>,
}

impl<E> DatetimeDeserializer<E> {
    /// Create a deserializer to emit [`Datetime`][crate::Datetime]
    pub fn new(date: crate::Datetime) -> Self {
        Self {
            date: Some(date),
            _error: Default::default(),
        }
    }
}

impl<'de, E> serde::de::MapAccess<'de> for DatetimeDeserializer<E>
where
    E: serde::de::Error,
{
    type Error = E;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        if self.date.is_some() {
            seed.deserialize(BorrowedStrDeserializer::new(crate::datetime::FIELD))
                .map(Some)
        } else {
            Ok(None)
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        if let Some(date) = self.date.take() {
            seed.deserialize(date.to_string().into_deserializer())
        } else {
            panic!("next_value_seed called before next_key_seed")
        }
    }
}

/// Integrate [`Datetime`][crate::Datetime] into an untagged deserialize
#[cfg(feature = "alloc")]
pub enum VisitMap<'de> {
    /// The map was deserialized as a [Datetime][crate::Datetime] value
    Datetime(crate::Datetime),
    /// The map is of an unknown format and needs further deserialization
    Key(alloc::borrow::Cow<'de, str>),
}

impl<'de> VisitMap<'de> {
    /// Determine the type of the map by deserializing it
    pub fn next_key_seed<V: serde::de::MapAccess<'de>>(
        visitor: &mut V,
    ) -> Result<Option<Self>, V::Error> {
        let mut key = None;
        let Some(()) = visitor.next_key_seed(DatetimeOrTable::new(&mut key))? else {
            return Ok(None);
        };
        let result = if let Some(key) = key {
            VisitMap::Key(key)
        } else {
            let date: crate::datetime::DatetimeFromString = visitor.next_value()?;
            VisitMap::Datetime(date.value)
        };
        Ok(Some(result))
    }
}

struct DatetimeOrTable<'m, 'de> {
    key: &'m mut Option<alloc::borrow::Cow<'de, str>>,
}

impl<'m, 'de> DatetimeOrTable<'m, 'de> {
    fn new(key: &'m mut Option<alloc::borrow::Cow<'de, str>>) -> Self {
        *key = None;
        Self { key }
    }
}

impl<'de> serde::de::DeserializeSeed<'de> for DatetimeOrTable<'_, 'de> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de> serde::de::Visitor<'de> for DatetimeOrTable<'_, 'de> {
    type Value = ();

    fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("a string key")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if s == crate::datetime::FIELD {
            *self.key = None;
            Ok(())
        } else {
            use crate::alloc::borrow::ToOwned as _;
            *self.key = Some(alloc::borrow::Cow::Owned(s.to_owned()));
            Ok(())
        }
    }

    fn visit_borrowed_str<E>(self, s: &'de str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if s == crate::datetime::FIELD {
            *self.key = None;
            Ok(())
        } else {
            *self.key = Some(alloc::borrow::Cow::Borrowed(s));
            Ok(())
        }
    }

    #[allow(unused_qualifications)]
    fn visit_string<E>(self, s: alloc::string::String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if s == crate::datetime::FIELD {
            *self.key = None;
            Ok(())
        } else {
            *self.key = Some(alloc::borrow::Cow::Owned(s));
            Ok(())
        }
    }
}
