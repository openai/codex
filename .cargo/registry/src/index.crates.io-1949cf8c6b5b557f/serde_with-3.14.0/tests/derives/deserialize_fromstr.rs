use super::*;
use core::{
    num::ParseIntError,
    str::{FromStr, ParseBoolError},
};
use pretty_assertions::assert_eq;
use serde_with::DeserializeFromStr;

#[derive(Debug, PartialEq, DeserializeFromStr)]
struct A {
    a: u32,
    b: bool,
}

impl FromStr for A {
    type Err = String;

    /// Parse a value like `123<>true`
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split("<>");
        let number = parts
            .next()
            .ok_or_else(|| "Missing first value".to_string())?
            .parse()
            .map_err(|err: ParseIntError| err.to_string())?;
        let bool = parts
            .next()
            .ok_or_else(|| "Missing second value".to_string())?
            .parse()
            .map_err(|err: ParseBoolError| err.to_string())?;
        Ok(Self { a: number, b: bool })
    }
}

#[test]
fn test_deserialize_fromstr() {
    check_deserialization(A { a: 159, b: true }, "\"159<>true\"");
    check_deserialization(A { a: 999, b: false }, "\"999<>false\"");
    check_deserialization(A { a: 0, b: true }, "\"0<>true\"");
}

#[test]
fn test_deserialize_from_bytes() {
    use serde::de::{value::Error, Deserialize, Deserializer, Visitor};

    // Unfortunately serde_json is too clever (i.e. handles bytes gracefully)
    // so instead create a custom deserializer which can only deserialize bytes.
    // All other deserialize_* fns are forwarded to deserialize_bytes
    struct ByteDeserializer(&'static [u8]);

    impl<'de> Deserializer<'de> for ByteDeserializer {
        type Error = Error;

        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_bytes(visitor)
        }

        fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
        {
            visitor.visit_bytes(self.0)
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
            byte_buf option unit unit_struct newtype_struct seq tuple
            tuple_struct map struct enum identifier ignored_any
        }
    }

    // callstack: A::deserialize -> deserialize_str -> deserialize_any ->
    // deserialize_bytes -> visit_bytes -> visit_str -> success!
    let a = A::deserialize(ByteDeserializer(b"159<>true")).unwrap();

    assert_eq!(A { a: 159, b: true }, a);
}

#[test]
fn test_deserialize_fromstr_in_vec() {
    check_deserialization(
        vec![
            A { a: 123, b: false },
            A { a: 0, b: true },
            A { a: 999, b: true },
        ],
        r#"[
        "123<>false",
        "0<>true",
        "999<>true"
      ]"#,
    );
}
