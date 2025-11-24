use crate::de::Error;

/// Deserializes table values into enum variants.
pub(crate) struct TableEnumDeserializer {
    value: crate::Item,
}

impl TableEnumDeserializer {
    pub(crate) fn new(value: crate::Item) -> Self {
        Self { value }
    }
}

impl<'de> serde::de::VariantAccess<'de> for TableEnumDeserializer {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        match self.value {
            crate::Item::ArrayOfTables(values) => {
                if values.is_empty() {
                    Ok(())
                } else {
                    Err(Error::custom("expected empty array", values.span()))
                }
            }
            crate::Item::Value(crate::Value::Array(values)) => {
                if values.is_empty() {
                    Ok(())
                } else {
                    Err(Error::custom("expected empty table", values.span()))
                }
            }
            crate::Item::Table(values) => {
                if values.is_empty() {
                    Ok(())
                } else {
                    Err(Error::custom("expected empty table", values.span()))
                }
            }
            crate::Item::Value(crate::Value::InlineTable(values)) => {
                if values.is_empty() {
                    Ok(())
                } else {
                    Err(Error::custom("expected empty table", values.span()))
                }
            }
            e => Err(Error::custom(
                format!("expected table, found {}", e.type_name()),
                e.span(),
            )),
        }
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        seed.deserialize(super::ValueDeserializer::new(self.value))
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self.value {
            crate::Item::ArrayOfTables(values) => {
                let values_span = values.span();
                let tuple_values = values.values.into_iter().collect::<Vec<_>>();

                if tuple_values.len() == len {
                    serde::de::Deserializer::deserialize_seq(
                        super::ArrayDeserializer::new(tuple_values, values_span),
                        visitor,
                    )
                } else {
                    Err(Error::custom(
                        format!("expected tuple with length {len}"),
                        values_span,
                    ))
                }
            }
            crate::Item::Value(crate::Value::Array(values)) => {
                let values_span = values.span();
                let tuple_values = values.values.into_iter().collect::<Vec<_>>();

                if tuple_values.len() == len {
                    serde::de::Deserializer::deserialize_seq(
                        super::ArrayDeserializer::new(tuple_values, values_span),
                        visitor,
                    )
                } else {
                    Err(Error::custom(
                        format!("expected tuple with length {len}"),
                        values_span,
                    ))
                }
            }
            crate::Item::Table(values) => {
                let values_span = values.span();
                let tuple_values: Result<Vec<_>, _> = values
                    .items
                    .into_iter()
                    .enumerate()
                    .map(|(index, (key, value))| match key.get().parse::<usize>() {
                        Ok(key_index) if key_index == index => Ok(value),
                        Ok(_) | Err(_) => Err(Error::custom(
                            format!("expected table key `{}`, but was `{}`", index, key.get()),
                            key.span(),
                        )),
                    })
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
                        values_span,
                    ))
                }
            }
            crate::Item::Value(crate::Value::InlineTable(values)) => {
                let values_span = values.span();
                let tuple_values: Result<Vec<_>, _> = values
                    .items
                    .into_iter()
                    .enumerate()
                    .map(|(index, (key, value))| match key.get().parse::<usize>() {
                        Ok(key_index) if key_index == index => Ok(value),
                        Ok(_) | Err(_) => Err(Error::custom(
                            format!("expected table key `{}`, but was `{}`", index, key.get()),
                            key.span(),
                        )),
                    })
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
                        values_span,
                    ))
                }
            }
            e => Err(Error::custom(
                format!("expected table, found {}", e.type_name()),
                e.span(),
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
            super::ValueDeserializer::new(self.value).with_struct_key_validation(),
            "", // TODO: this should be the variant name
            fields,
            visitor,
        )
    }
}
