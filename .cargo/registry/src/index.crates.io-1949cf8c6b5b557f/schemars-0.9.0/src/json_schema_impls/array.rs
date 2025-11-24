use crate::SchemaGenerator;
use crate::_alloc_prelude::*;
use crate::{json_schema, JsonSchema, Schema};
use alloc::borrow::Cow;

// Does not require T: JsonSchema.
impl<T> JsonSchema for [T; 0] {
    inline_schema!();

    fn schema_name() -> Cow<'static, str> {
        "EmptyArray".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "[]".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "array",
            "maxItems": 0,
        })
    }
}

macro_rules! array_impls {
    ($($len:tt)+) => {
        $(
            impl<T: JsonSchema> JsonSchema for [T; $len] {
                inline_schema!();

                fn schema_name() -> Cow<'static, str> {
                    format!("Array_size_{}_of_{}", $len, T::schema_name()).into()
                }

                fn schema_id() -> Cow<'static, str> {
                    format!("[{}; {}]", $len, T::schema_id()).into()
                }

                fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                    json_schema!({
                        "type": "array",
                        "items": serde_json::Value::from(generator.subschema_for::<T>()),
                        "minItems": $len,
                        "maxItems": $len,
                    })
                }
            }
        )+
    }
}

array_impls! {
     1  2  3  4  5  6  7  8  9 10
    11 12 13 14 15 16 17 18 19 20
    21 22 23 24 25 26 27 28 29 30
    31 32
}
