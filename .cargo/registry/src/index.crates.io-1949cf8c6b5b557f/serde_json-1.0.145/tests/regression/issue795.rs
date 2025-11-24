#![allow(clippy::assertions_on_result_states)]

use serde::de::{
    Deserialize, Deserializer, EnumAccess, IgnoredAny, MapAccess, VariantAccess, Visitor,
};
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum Enum {
    Variant {
        #[allow(dead_code)]
        x: u8,
    },
}

impl<'de> Deserialize<'de> for Enum {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct EnumVisitor;

        impl<'de> Visitor<'de> for EnumVisitor {
            type Value = Enum;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("enum Enum")
            }

            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
            where
                A: EnumAccess<'de>,
            {
                let (IgnoredAny, variant) = data.variant()?;
                variant.struct_variant(&["x"], self)
            }

            fn visit_map<A>(self, mut data: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut x = 0;
                if let Some((IgnoredAny, value)) = data.next_entry()? {
                    x = value;
                }
                Ok(Enum::Variant { x })
            }
        }

        deserializer.deserialize_enum("Enum", &["Variant"], EnumVisitor)
    }
}

#[test]
fn test() {
    let s = r#" {"Variant":{"x":0,"y":0}} "#;
    assert!(serde_json::from_str::<Enum>(s).is_err());

    let j = json!({"Variant":{"x":0,"y":0}});
    assert!(serde_json::from_value::<Enum>(j).is_err());
}
