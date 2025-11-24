use crate::SchemaGenerator;
use crate::_alloc_prelude::*;
use crate::{json_schema, JsonSchema, Schema};
use alloc::borrow::Cow;
use either1::Either;

impl<L: JsonSchema, R: JsonSchema> JsonSchema for Either<L, R> {
    inline_schema!();

    fn schema_name() -> Cow<'static, str> {
        format!("Either_{}_or_{}", L::schema_name(), R::schema_name()).into()
    }

    fn schema_id() -> Cow<'static, str> {
        format!("either::Either<{}, {}>", L::schema_id(), R::schema_id()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
          "oneOf": [
            {
              "type": "object",
              "properties": {
                "Left": generator.subschema_for::<L>()
              },
              "additionalProperties": false,
              "required": [
                "Left"
              ]
            },
            {
              "type": "object",
              "properties": {
                "Right": generator.subschema_for::<R>()
              },
              "additionalProperties": false,
              "required": [
                "Right"
              ]
            }
          ]
        })
    }
}
