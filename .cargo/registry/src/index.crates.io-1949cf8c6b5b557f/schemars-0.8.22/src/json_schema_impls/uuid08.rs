use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use std::borrow::Cow;
use uuid08::Uuid;

impl JsonSchema for Uuid {
    no_ref_schema!();

    fn schema_name() -> String {
        "Uuid".to_string()
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("uuid::Uuid")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            format: Some("uuid".to_string()),
            ..Default::default()
        }
        .into()
    }
}
