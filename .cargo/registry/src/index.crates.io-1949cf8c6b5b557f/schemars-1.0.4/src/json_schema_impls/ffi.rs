use crate::SchemaGenerator;
use crate::_alloc_prelude::*;
use crate::{json_schema, JsonSchema, Schema};
use alloc::borrow::Cow;
use serde_json::json;
use std::ffi::{CStr, CString, OsStr, OsString};

impl JsonSchema for OsString {
    fn schema_name() -> Cow<'static, str> {
        "OsString".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "std::ffi::OsString".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "oneOf": [
                {
                    "type": "object",
                    "properties": {
                        "Unix": <Vec<u8>>::json_schema(generator)
                    },
                    "required": ["Unix"]
                },
                {
                    "type": "object",
                    "properties": {
                        "Windows": <Vec<u16>>::json_schema(generator)
                    },
                    "required": ["Windows"]
                },
            ]
        })
    }
}

forward_impl!(OsStr => OsString);

impl JsonSchema for CString {
    fn schema_name() -> Cow<'static, str> {
        "CString".into()
    }

    fn schema_id() -> Cow<'static, str> {
        "std::ffi::CString".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let ty = if generator.contract().is_deserialize() {
            json!(["array", "string"])
        } else {
            json!("array")
        };

        json_schema!({
            "type": ty,
            "items": {
                "type": "integer",
                "minimum": 1,
                "maximum": 255
            },
        })
    }
}

forward_impl!(CStr => CString);
