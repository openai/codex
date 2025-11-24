use crate::_alloc_prelude::*;
use crate::generate::Contract;
use crate::{JsonSchema, Schema};
use alloc::borrow::Cow;
use serde_json::Value;

impl JsonSchema for bytes1::Bytes {
    fn schema_name() -> Cow<'static, str> {
        "Bytes".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "bytes::Bytes".into()
    }

    fn json_schema(generator: &mut crate::SchemaGenerator) -> crate::Schema {
        let ty = match generator.contract() {
            Contract::Deserialize => Value::Array(vec!["array".into(), "string".into()]),
            Contract::Serialize => "array".into(),
        };

        let mut result = Schema::default();
        result.insert("type".to_owned(), ty);
        result.insert("items".to_owned(), generator.subschema_for::<u8>().into());
        result
    }
}

forward_impl!(bytes1::BytesMut => bytes1::Bytes);
