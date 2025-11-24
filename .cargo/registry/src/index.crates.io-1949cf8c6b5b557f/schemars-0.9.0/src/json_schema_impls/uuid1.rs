use crate::SchemaGenerator;
use crate::{json_schema, JsonSchema, Schema};
use alloc::borrow::Cow;
use uuid1::Uuid;

impl JsonSchema for Uuid {
    inline_schema!();

    fn schema_name() -> Cow<'static, str> {
        "Uuid".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "uuid::Uuid".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "uuid",
        })
    }
}
