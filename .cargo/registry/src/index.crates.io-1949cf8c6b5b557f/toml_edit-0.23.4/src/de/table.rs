use serde::de::IntoDeserializer;

use crate::de::Error;

pub(crate) struct TableDeserializer {
    span: Option<std::ops::Range<usize>>,
    items: crate::table::KeyValuePairs,
}

impl TableDeserializer {
    pub(crate) fn new(
        items: crate::table::KeyValuePairs,
        span: Option<std::ops::Range<usize>>,
    ) -> Self {
        Self { span, items }
    }
}

impl<'de> serde::Deserializer<'de> for TableDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_map(TableMapAccess::new(self))
    }

    // `None` is interpreted as a missing field so be sure to implement `Some`
    // as a present field.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if serde_spanned::de::is_spanned(name) {
            if let Some(span) = self.span.clone() {
                return visitor.visit_map(
                    serde_spanned::de::SpannedDeserializer::<Self, Error>::new(self, span),
                );
            } else {
                return Err(Error::custom("value is missing a span", None));
            }
        }

        self.deserialize_any(visitor)
    }

    // Called when the type to deserialize is an enum, as opposed to a field in the type.
    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: serde::de::Visitor<'de>,
    {
        if self.items.is_empty() {
            Err(Error::custom(
                "wanted exactly 1 element, found 0 elements",
                self.span,
            ))
        } else if self.items.len() != 1 {
            Err(Error::custom(
                "wanted exactly 1 element, more than 1 element",
                self.span,
            ))
        } else {
            visitor.visit_enum(TableMapAccess::new(self))
        }
    }

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map unit
        ignored_any unit_struct tuple_struct tuple identifier
    }
}

impl IntoDeserializer<'_, Error> for TableDeserializer {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

pub(crate) struct TableMapAccess {
    iter: indexmap::map::IntoIter<crate::Key, crate::Item>,
    span: Option<std::ops::Range<usize>>,
    value: Option<(crate::Key, crate::Item)>,
}

impl TableMapAccess {
    pub(crate) fn new(input: TableDeserializer) -> Self {
        Self {
            iter: input.items.into_iter(),
            span: input.span,
            value: None,
        }
    }
}

impl<'de> serde::de::MapAccess<'de> for TableMapAccess {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((k, v)) => {
                let key_span = k.span();
                let ret = seed
                    .deserialize(super::KeyDeserializer::new(k.clone(), key_span.clone()))
                    .map(Some)
                    .map_err(|mut e: Self::Error| {
                        if e.span().is_none() {
                            e.set_span(key_span);
                        }
                        e
                    });
                self.value = Some((k, v));
                ret
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        match self.value.take() {
            Some((k, v)) => {
                let span = v.span().or_else(|| k.span());
                seed.deserialize(crate::de::ValueDeserializer::new(v))
                    .map_err(|mut e: Self::Error| {
                        if e.span().is_none() {
                            e.set_span(span);
                        }
                        e.add_key(k.get().to_owned());
                        e
                    })
            }
            None => {
                panic!("no more values in next_value_seed, internal error in ValueDeserializer")
            }
        }
    }
}

impl<'de> serde::de::EnumAccess<'de> for TableMapAccess {
    type Error = Error;
    type Variant = super::TableEnumDeserializer;

    fn variant_seed<V>(mut self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let (key, value) = match self.iter.next() {
            Some(pair) => pair,
            None => {
                return Err(Error::custom(
                    "expected table with exactly 1 entry, found empty table",
                    self.span,
                ));
            }
        };

        let val = seed
            .deserialize(key.into_deserializer())
            .map_err(|mut e: Self::Error| {
                if e.span().is_none() {
                    e.set_span(key.span());
                }
                e
            })?;

        let variant = super::TableEnumDeserializer::new(value);

        Ok((val, variant))
    }
}
