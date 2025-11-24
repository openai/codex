use serde::de::IntoDeserializer;
use serde_spanned::Spanned;

use crate::de::DeString;
use crate::de::DeTable;
use crate::de::DeValue;
use crate::de::Error;
use crate::map::IntoIter;

pub(crate) struct TableDeserializer<'i> {
    span: core::ops::Range<usize>,
    items: DeTable<'i>,
}

impl<'i> TableDeserializer<'i> {
    pub(crate) fn new(items: DeTable<'i>, span: core::ops::Range<usize>) -> Self {
        Self { span, items }
    }
}

impl<'de> serde::Deserializer<'de> for TableDeserializer<'de> {
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
            let span = self.span.clone();
            return visitor.visit_map(super::SpannedDeserializer::new(self, span));
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
                Some(self.span),
            ))
        } else if self.items.len() != 1 {
            Err(Error::custom(
                "wanted exactly 1 element, more than 1 element",
                Some(self.span),
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

impl<'de> IntoDeserializer<'de, Error> for TableDeserializer<'de> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

pub(crate) struct TableMapAccess<'i> {
    iter: IntoIter<Spanned<DeString<'i>>, Spanned<DeValue<'i>>>,
    span: core::ops::Range<usize>,
    value: Option<(Spanned<DeString<'i>>, Spanned<DeValue<'i>>)>,
}

impl<'i> TableMapAccess<'i> {
    pub(crate) fn new(input: TableDeserializer<'i>) -> Self {
        Self {
            iter: input.items.into_iter(),
            span: input.span,
            value: None,
        }
    }
}

impl<'de> serde::de::MapAccess<'de> for TableMapAccess<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: serde::de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some((k, v)) => {
                let key_span = k.span();
                let ret = seed
                    .deserialize(super::KeyDeserializer::new(
                        k.clone().into_inner(),
                        Some(key_span.clone()),
                    ))
                    .map(Some)
                    .map_err(|mut e: Self::Error| {
                        if e.span().is_none() {
                            e.set_span(Some(key_span));
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
                let span = v.span();
                seed.deserialize(crate::de::ValueDeserializer::with_parts(
                    v.into_inner(),
                    span.clone(),
                ))
                .map_err(|mut e: Self::Error| {
                    if e.span().is_none() {
                        e.set_span(Some(span));
                    }
                    e.add_key(k.into_inner().into_owned());
                    e
                })
            }
            None => {
                panic!("no more values in next_value_seed, internal error in ValueDeserializer")
            }
        }
    }
}

impl<'de> serde::de::EnumAccess<'de> for TableMapAccess<'de> {
    type Error = Error;
    type Variant = super::TableEnumDeserializer<'de>;

    fn variant_seed<V>(mut self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: serde::de::DeserializeSeed<'de>,
    {
        let (key, value) = match self.iter.next() {
            Some(pair) => pair,
            None => {
                return Err(Error::custom(
                    "expected table with exactly 1 entry, found empty table",
                    Some(self.span),
                ));
            }
        };

        let key_span = key.span();
        let val = seed
            .deserialize(super::KeyDeserializer::new(
                key.into_inner(),
                Some(key_span.clone()),
            ))
            .map_err(|mut e: Self::Error| {
                if e.span().is_none() {
                    e.set_span(Some(key_span));
                }
                e
            })?;

        let value_span = value.span();
        let value = value.into_inner();
        let variant = super::TableEnumDeserializer::new(value, value_span);

        Ok((val, variant))
    }
}
