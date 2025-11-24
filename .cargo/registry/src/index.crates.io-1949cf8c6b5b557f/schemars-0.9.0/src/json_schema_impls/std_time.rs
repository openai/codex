use crate::SchemaGenerator;
use crate::{json_schema, JsonSchema, Schema};
use alloc::borrow::Cow;

impl JsonSchema for core::time::Duration {
    fn schema_name() -> Cow<'static, str> {
        "Duration".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "std::time::Duration".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "required": ["secs", "nanos"],
            "properties": {
                "secs": u64::json_schema(generator),
                "nanos": u32::json_schema(generator),
            }
        })
    }
}

#[cfg(feature = "std")]
impl JsonSchema for std::time::SystemTime {
    fn schema_name() -> Cow<'static, str> {
        "SystemTime".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "std::time::SystemTime".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "required": ["secs_since_epoch", "nanos_since_epoch"],
            "properties": {
                "secs_since_epoch": u64::json_schema(generator),
                "nanos_since_epoch": u32::json_schema(generator),
            }
        })
    }
}
