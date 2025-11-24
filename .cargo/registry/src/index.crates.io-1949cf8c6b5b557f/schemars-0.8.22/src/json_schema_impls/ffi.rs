use crate::r#gen::SchemaGenerator;
use crate::schema::*;
use crate::JsonSchema;
use std::borrow::Cow;
use std::ffi::{CStr, CString, OsStr, OsString};

impl JsonSchema for OsString {
    fn schema_name() -> String {
        "OsString".to_owned()
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed("std::ffi::OsString")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut unix_schema = SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            ..Default::default()
        };
        let obj = unix_schema.object();
        obj.required.insert("Unix".to_owned());
        obj.properties
            .insert("Unix".to_owned(), <Vec<u8>>::json_schema(generator));

        let mut win_schema = SchemaObject {
            instance_type: Some(InstanceType::Object.into()),
            ..Default::default()
        };
        let obj = win_schema.object();
        obj.required.insert("Windows".to_owned());
        obj.properties
            .insert("Windows".to_owned(), <Vec<u16>>::json_schema(generator));

        let mut schema = SchemaObject::default();
        schema.subschemas().one_of = Some(vec![unix_schema.into(), win_schema.into()]);
        schema.into()
    }
}

forward_impl!(OsStr => OsString);

forward_impl!(CString => Vec<u8>);
forward_impl!(CStr => Vec<u8>);
