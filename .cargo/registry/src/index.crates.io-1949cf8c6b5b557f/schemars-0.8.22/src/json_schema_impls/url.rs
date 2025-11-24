use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use std::borrow::Cow;
use url::Url;

impl JsonSchema for Url {
    no_ref_schema!();

    fn schema_name() -> String {
        "Url".to_owned()
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("url::Url")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            format: Some("uri".to_owned()),
            ..Default::default()
        }
        .into()
    }
}
