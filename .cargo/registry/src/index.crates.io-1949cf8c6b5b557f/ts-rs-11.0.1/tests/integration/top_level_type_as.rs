use std::collections::HashMap;

#[cfg(feature = "serde-json-impl")]
use serde_json::Value as JsonValue;
use ts_rs::TS;

#[derive(TS)]
#[ts(as = "T")]
pub enum UntaggedEnum<T: TS> {
    Left(T),
    Right(T),
}

#[test]
pub fn top_level_type_as_enum() {
    assert_eq!(UntaggedEnum::<String>::inline(), r#"string"#)
}

#[derive(TS)]
#[ts(as = "T")]
pub struct Wrapper<T: TS>(T);

#[test]
pub fn top_level_type_as_struct() {
    assert_eq!(Wrapper::<String>::inline(), r#"string"#)
}

#[cfg(feature = "serde-json-impl")]
#[derive(TS)]
#[ts(
    export,
    export_to = "top_level_type_as/",
    as = "HashMap::<String, JsonValue>"
)]
pub struct JsonMap(JsonValue);

#[derive(TS)]
#[ts(export, export_to = "top_level_type_as/")]
pub struct Foo {
    x: i32,
}

#[derive(TS)]
#[ts(export, export_to = "top_level_type_as/")]
pub struct Bar {
    foo: Foo,
}

#[derive(TS)]
#[ts(
    export,
    export_to = "top_level_type_as/",
    as = "HashMap::<String, Bar>"
)]
pub struct Biz(String);
