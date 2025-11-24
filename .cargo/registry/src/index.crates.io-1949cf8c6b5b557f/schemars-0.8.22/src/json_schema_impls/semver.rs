use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use semver::Version;
use std::borrow::Cow;

impl JsonSchema for Version {
    no_ref_schema!();

    fn schema_name() -> String {
        "Version".to_owned()
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("semver::Version")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            string: Some(Box::new(StringValidation {
                // https://semver.org/#is-there-a-suggested-regular-expression-regex-to-check-a-semver-string
                pattern: Some(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$".to_owned()),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}
