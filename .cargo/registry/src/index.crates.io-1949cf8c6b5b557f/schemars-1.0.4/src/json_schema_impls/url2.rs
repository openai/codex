use crate::SchemaGenerator;
use crate::{json_schema, JsonSchema, Schema};
use alloc::borrow::Cow;
use url2::Url;

impl JsonSchema for Url {
    inline_schema!();

    fn schema_name() -> Cow<'static, str> {
        "Url".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "url::Url".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "uri",
        })
    }
}
