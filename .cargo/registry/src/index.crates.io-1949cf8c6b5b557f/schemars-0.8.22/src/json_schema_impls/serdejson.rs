use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use serde_json::{Map, Number, Value};
use std::borrow::Cow;
use std::collections::BTreeMap;

impl JsonSchema for Value {
    no_ref_schema!();

    fn schema_name() -> String {
        "AnyValue".to_owned()
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("AnyValue")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        Schema::Bool(true)
    }
}

forward_impl!(Map<String, Value> => BTreeMap<String, Value>);

impl JsonSchema for Number {
    no_ref_schema!();

    fn schema_name() -> String {
        "Number".to_owned()
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("Number")
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::Number.into()),
            ..Default::default()
        }
        .into()
    }
}

#[cfg(feature = "raw_value")]
forward_impl!(serde_json::value::RawValue => Value);
