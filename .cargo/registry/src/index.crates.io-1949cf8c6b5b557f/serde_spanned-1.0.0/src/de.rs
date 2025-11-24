//! Deserialzation support for [`Spanned`]

use serde::de::value::BorrowedStrDeserializer;
use serde::de::IntoDeserializer as _;

use crate::Spanned;

/// Check if deserializing a [`Spanned`]
pub fn is_spanned(name: &'static str) -> bool {
    crate::spanned::is_spanned(name)
}

/// Deserializer / format support for emitting [`Spanned`]
pub struct SpannedDeserializer<'de, T, E>
where
    T: serde::de::IntoDeserializer<'de, E>,
    E: serde::de::Error,
{
    start: Option<usize>,
    end: Option<usize>,
    value: Option<T>,
    _lifetime: core::marker::PhantomData<&'de ()>,
    _error: core::marker::PhantomData<E>,
}

impl<'de, T, E> SpannedDeserializer<'de, T, E>
where
    T: serde::de::IntoDeserializer<'de, E>,
    E: serde::de::Error,
{
    /// Create a deserializer to emit [`Spanned`]
    pub fn new(value: T, span: core::ops::Range<usize>) -> Self {
        Self {
            start: Some(span.start),
            end: Some(span.end),
            value: Some(value),
            _lifetime: Default::default(),
            _error: Default::default(),
        }
    }
}

impl<'de, T, E> serde::de::MapAccess<'de> for SpannedDeserializer<'de, T, E>
where
    T: serde::de::IntoDeserializer<'de, E>,
    E: serde::de::Error,
{
    type Error = E;
    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        if self.start.is_some() {
            seed.deserialize(BorrowedStrDeserializer::new(Spanned::<T>::START_FIELD))
                .map(Some)
        } else if self.end.is_some() {
            seed.deserialize(BorrowedStrDeserializer::new(Spanned::<T>::END_FIELD))
                .map(Some)
        } else if self.value.is_some() {
            seed.deserialize(BorrowedStrDeserializer::new(Spanned::<T>::VALUE_FIELD))
                .map(Some)
        } else {
            Ok(None)
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        if let Some(start) = self.start.take() {
            seed.deserialize(start.into_deserializer())
        } else if let Some(end) = self.end.take() {
            seed.deserialize(end.into_deserializer())
        } else if let Some(value) = self.value.take() {
            seed.deserialize(value.into_deserializer())
        } else {
            panic!("next_value_seed called before next_key_seed")
        }
    }
}
