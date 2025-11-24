use crate::_alloc_prelude::*;
use crate::{json_schema, JsonSchema, Schema, SchemaGenerator};
use alloc::borrow::Cow;
use alloc::collections::BTreeMap;
use serde_json::{json, Value};

impl<K, V> JsonSchema for BTreeMap<K, V>
where
    K: JsonSchema,
    V: JsonSchema,
{
    inline_schema!();

    fn schema_name() -> Cow<'static, str> {
        Cow::Owned(if K::schema_id() == <str>::schema_id() {
            format!("Map_of_{}", V::schema_name())
        } else {
            format!("Map_from_{}_to_{}", K::schema_name(), V::schema_name())
        })
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Owned(if K::schema_id() == <str>::schema_id() {
            format!("Map<{}>", V::schema_id())
        } else {
            format!("Map<{}, {}>", K::schema_id(), V::schema_id())
        })
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let key_schema = K::json_schema(generator);
        let value_schema = generator.subschema_for::<V>();

        let mut map_schema = json_schema!({
            "type": "object",
        });

        let key_pattern = key_schema.get("pattern").and_then(Value::as_str);
        let key_type = key_schema.get("type").and_then(Value::as_str);
        let key_minimum = key_schema.get("minimum").and_then(Value::as_u64);

        let pattern = match (key_pattern, key_type) {
            (Some(pattern), Some("string")) => pattern,
            (_, Some("integer")) if key_minimum == Some(0) => r"^\d+$",
            (_, Some("integer")) => r"^-?\d+$",
            _ => {
                map_schema.insert("additionalProperties".to_owned(), value_schema.to_value());
                return map_schema;
            }
        };

        map_schema.insert(
            "patternProperties".to_owned(),
            json!({ pattern: value_schema }),
        );
        map_schema.insert("additionalProperties".to_owned(), Value::Bool(false));

        map_schema
    }
}

#[cfg(feature = "std")]
forward_impl!((<K: JsonSchema, V: JsonSchema, H> JsonSchema for std::collections::HashMap<K, V, H>) => BTreeMap<K, V>);
