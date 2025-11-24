use crate::de::Error;

pub(crate) struct ArrayDeserializer {
    input: Vec<crate::Item>,
    span: Option<std::ops::Range<usize>>,
}

impl ArrayDeserializer {
    pub(crate) fn new(input: Vec<crate::Item>, span: Option<std::ops::Range<usize>>) -> Self {
        Self { input, span }
    }
}

impl<'de> serde::Deserializer<'de> for ArrayDeserializer {
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

    serde::forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string seq
        bytes byte_buf map option unit newtype_struct
        ignored_any unit_struct tuple_struct tuple enum identifier
    }
}

impl serde::de::IntoDeserializer<'_, Error> for ArrayDeserializer {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}

pub(crate) struct ArraySeqAccess {
    iter: std::vec::IntoIter<crate::Item>,
}

impl ArraySeqAccess {
    pub(crate) fn new(input: Vec<crate::Item>) -> Self {
        Self {
            iter: input.into_iter(),
        }
    }
}

impl<'de> serde::de::SeqAccess<'de> for ArraySeqAccess {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        match self.iter.next() {
            Some(v) => seed
                .deserialize(crate::de::ValueDeserializer::new(v))
                .map(Some),
            None => Ok(None),
        }
    }
}
