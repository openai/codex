use crate::_alloc_prelude::*;
use crate::generate::Contract;
use crate::{JsonSchema, Schema, SchemaGenerator};
use alloc::borrow::Cow;
use serde_json::Value;

macro_rules! decimal_impl {
    ($type:ty) => {
        impl JsonSchema for $type {
            inline_schema!();

            fn schema_name() -> Cow<'static, str> {
                "Decimal".into()
            }

            fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                let (ty, pattern) = match generator.contract() {
                    Contract::Deserialize => (
                        Value::Array(vec!["string".into(), "number".into()]),
                        r"^-?[0-9]+(\.[0-9]+)?([eE][0-9]+)?$".into(),
                    ),
                    Contract::Serialize => ("string".into(), r"^-?[0-9]+(\.[0-9]+)?$".into()),
                };

                let mut result = Schema::default();
                result.insert("type".to_owned(), ty);
                result.insert("pattern".to_owned(), pattern);
                result
            }
        }
    };
}

#[cfg(feature = "rust_decimal1")]
decimal_impl!(rust_decimal1::Decimal);
#[cfg(feature = "bigdecimal04")]
decimal_impl!(bigdecimal04::BigDecimal);
