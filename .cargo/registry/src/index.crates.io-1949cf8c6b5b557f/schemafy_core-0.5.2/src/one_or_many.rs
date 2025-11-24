pub type OneOrMany<T> = Vec<T>;

pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    use std::fmt;
    use std::marker::PhantomData;

    use serde::de::value::{MapAccessDeserializer, SeqAccessDeserializer};
    use serde::de::{self, Deserialize, IntoDeserializer};

    struct OneOrManyDeserializer<T>(PhantomData<T>);
    impl<'de2, T> serde::de::Visitor<'de2> for OneOrManyDeserializer<T>
    where
        T: Deserialize<'de2>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("one or many")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Vec<T>, E>
        where
            E: de::Error,
        {
            Deserialize::deserialize(value.into_deserializer()).map(|v| vec![v])
        }

        fn visit_u64<E>(self, value: u64) -> Result<Vec<T>, E>
        where
            E: de::Error,
        {
            Deserialize::deserialize(value.into_deserializer()).map(|v| vec![v])
        }

        fn visit_str<E>(self, value: &str) -> Result<Vec<T>, E>
        where
            E: de::Error,
        {
            Deserialize::deserialize(value.into_deserializer()).map(|v| vec![v])
        }

        fn visit_string<E>(self, value: String) -> Result<Vec<T>, E>
        where
            E: de::Error,
        {
            Deserialize::deserialize(value.into_deserializer()).map(|v| vec![v])
        }

        fn visit_map<V>(self, visitor: V) -> Result<Self::Value, V::Error>
        where
            V: serde::de::MapAccess<'de2>,
        {
            Deserialize::deserialize(MapAccessDeserializer::new(visitor)).map(|v| vec![v])
        }

        fn visit_seq<V>(self, visitor: V) -> Result<Self::Value, V::Error>
        where
            V: serde::de::SeqAccess<'de2>,
        {
            Deserialize::deserialize(SeqAccessDeserializer::new(visitor))
        }
    }
    deserializer.deserialize_any(OneOrManyDeserializer(PhantomData::<T>))
}

pub fn serialize<T, S>(value: &[T], serializer: S) -> Result<S::Ok, S::Error>
where
    T: serde::Serialize,
    S: serde::Serializer,
{
    use serde::Serialize;
    if value.len() == 1 {
        value[0].serialize(serializer)
    } else {
        value.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde::{Deserialize, Serialize};
    use serde_json::from_str;

    #[derive(PartialEq, Debug, Deserialize, Serialize)]
    struct OneOrMany<T>(
        #[serde(serialize_with = "serialize", deserialize_with = "deserialize")] Vec<T>,
    )
    where
        T: for<'de2> Deserialize<'de2> + Serialize;

    #[test]
    fn deserialize_one_int() {
        assert_eq!(from_str::<OneOrMany<i32>>("1").unwrap(), OneOrMany(vec![1]));
    }

    #[test]
    fn deserialize_many_int() {
        assert_eq!(
            from_str::<OneOrMany<i32>>("[1, 2, 3]").unwrap(),
            OneOrMany(vec![1, 2, 3])
        );
    }

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct Test {
        x: i32,
        y: Option<String>,
    }

    #[test]
    fn deserialize_one_struct() {
        assert_eq!(
            from_str::<OneOrMany<Test>>(r#"{ "x" : 10, "y" : "test" }"#).unwrap(),
            OneOrMany(vec![Test {
                x: 10,
                y: Some("test".to_string()),
            },])
        );
    }

    #[test]
    fn deserialize_one_struct_missing_field() {
        assert_eq!(
            from_str::<OneOrMany<Test>>(r#"{ "x" : 10 }"#).unwrap(),
            OneOrMany(vec![Test { x: 10, y: None }])
        );
    }

    #[test]
    fn deserialize_many_struct() {
        assert_eq!(
            from_str::<OneOrMany<Test>>(r#"[{ "x" : 10 }, { "x" : 0, "y" : "a" }]"#).unwrap(),
            OneOrMany(vec![
                Test { x: 10, y: None },
                Test {
                    x: 0,
                    y: Some("a".to_string()),
                },
            ])
        );
    }
}
