use crate::alloc_prelude::*;
use crate::de::DeArray;
use crate::de::DeValue;
use crate::de::Error;

/// Deserializes table values into enum variants.
pub(crate) struct TableEnumDeserializer<'i> {
    value: DeValue<'i>,
    span: core::ops::Range<usize>,
}

impl<'i> TableEnumDeserializer<'i> {
    pub(crate) fn new(value: DeValue<'i>, span: core::ops::Range<usize>) -> Self {
        TableEnumDeserializer { value, span }
    }
}

impl<'de> serde::de::VariantAccess<'de> for TableEnumDeserializer<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        match self.value {
            DeValue::Array(values) => {
                if values.is_empty() {
                    Ok(())
                } else {
                    Err(Error::custom("expected empty array", Some(self.span)))
                }
            }
            DeValue::Table(values) => {
                if values.is_empty() {
                    Ok(())
                } else {
                    Err(Error::custom("expected empty table", Some(self.span)))
                }
            }
            e => Err(Error::custom(
                format!("expected table, found {}", e.type_str()),
                Some(self.span),
            )),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        seed.deserialize(super::ValueDeserializer::with_parts(self.value, self.span))
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self.value {
            DeValue::Array(values) => {
                let values_span = self.span.clone();
                let tuple_values = values;

                if tuple_values.len() == len {
                    serde::de::Deserializer::deserialize_seq(
                        super::ArrayDeserializer::new(tuple_values, values_span),
                        visitor,
                    )
                } else {
                    Err(Error::custom(
                        format!("expected tuple with length {len}"),
                        Some(values_span),
                    ))
                }
            }
            DeValue::Table(values) => {
                let values_span = self.span.clone();
                let tuple_values: Result<DeArray<'_>, _> = values
                    .into_iter()
                    .enumerate()
                    .map(
                        |(index, (key, value))| match key.get_ref().parse::<usize>() {
                            Ok(key_index) if key_index == index => Ok(value),
                            Ok(_) | Err(_) => Err(Error::custom(
                                format!("expected table key `{index}`, but was `{key}`"),
                                Some(key.span()),
                            )),
                        },
                    )
                    .collect();
                let tuple_values = tuple_values?;

                if tuple_values.len() == len {
                    serde::de::Deserializer::deserialize_seq(
                        super::ArrayDeserializer::new(tuple_values, values_span),
                        visitor,
                    )
                } else {
                    Err(Error::custom(
                        format!("expected tuple with length {len}"),
                        Some(values_span),
                    ))
                }
            }
            e => Err(Error::custom(
                format!("expected table, found {}", e.type_str()),
                Some(self.span),
            )),
        }
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        serde::de::Deserializer::deserialize_struct(
            super::ValueDeserializer::with_parts(self.value, self.span)
                .with_struct_key_validation(),
            "", // TODO: this should be the variant name
            fields,
            visitor,
        )
    }
}
