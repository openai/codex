use serde_spanned::Spanned;

use crate::de::DeArray;
use crate::de::DeValue;
use crate::de::Error;

pub(crate) struct ArrayDeserializer<'i> {
    input: DeArray<'i>,
    span: core::ops::Range<usize>,
}

impl<'i> ArrayDeserializer<'i> {
    pub(crate) fn new(input: DeArray<'i>, span: core::ops::Range<usize>) -> Self {
        Self { input, span }
    }
}

impl<'de> serde::Deserializer<'de> for ArrayDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_seq(ArraySeqAccess::new(self.input))
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

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map option unit newtype_struct
        ignored_any unit_struct tuple_struct tuple enum identifier
    }
}

impl<'de> serde::de::IntoDeserializer<'de, Error> for ArrayDeserializer<'de> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

pub(crate) struct ArraySeqAccess<'i> {
    iter: alloc::vec::IntoIter<Spanned<DeValue<'i>>>,
}

impl<'i> ArraySeqAccess<'i> {
    pub(crate) fn new(input: DeArray<'i>) -> Self {
        Self {
            iter: input.into_iter(),
        }
    }
}

impl<'de> serde::de::SeqAccess<'de> for ArraySeqAccess<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(v) => {
                let span = v.span();
                let v = v.into_inner();
                seed.deserialize(crate::de::ValueDeserializer::with_parts(v, span))
                    .map(Some)
            }
            None => Ok(None),
        }
    }
}
