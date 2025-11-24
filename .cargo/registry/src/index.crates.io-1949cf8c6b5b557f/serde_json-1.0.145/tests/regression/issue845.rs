#![allow(clippy::trait_duplication_in_bounds)] // https://github.com/rust-lang/rust-clippy/issues/8757

use serde::{Deserialize, Deserializer};
use std::fmt::{self, Display};
use std::marker::PhantomData;
use std::str::FromStr;

pub struct NumberVisitor<T> {
    marker: PhantomData<T>,
}

impl<'de, T> serde::de::Visitor<'de> for NumberVisitor<T>
where
    T: TryFrom<u64> + TryFrom<i64> + FromStr,
    <T as TryFrom<u64>>::Error: Display,
    <T as TryFrom<i64>>::Error: Display,
    <T as FromStr>::Err: Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an integer or string")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        T::try_from(v).map_err(serde::de::Error::custom)
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        T::try_from(v).map_err(serde::de::Error::custom)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        v.parse().map_err(serde::de::Error::custom)
    }
}

fn deserialize_integer_or_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: TryFrom<u64> + TryFrom<i64> + FromStr,
    <T as TryFrom<u64>>::Error: Display,
    <T as TryFrom<i64>>::Error: Display,
    <T as FromStr>::Err: Display,
{
    deserializer.deserialize_any(NumberVisitor {
        marker: PhantomData,
    })
}

#[derive(Deserialize, Debug)]
pub struct Struct {
    #[serde(deserialize_with = "deserialize_integer_or_string")]
    #[allow(dead_code)]
    pub i: i64,
}

#[test]
fn test() {
    let j = r#" {"i":100} "#;
    println!("{:?}", serde_json::from_str::<Struct>(j).unwrap());

    let j = r#" {"i":"100"} "#;
    println!("{:?}", serde_json::from_str::<Struct>(j).unwrap());
}
