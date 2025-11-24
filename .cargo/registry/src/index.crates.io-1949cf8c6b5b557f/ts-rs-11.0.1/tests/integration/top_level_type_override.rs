use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "top_level_type_override/")]
#[ts(type = "string")]
#[non_exhaustive]
pub enum IncompleteEnum {
    Foo,
    Bar,
    Baz,
    // more
}

#[test]
pub fn top_level_type_override_enum() {
    assert_eq!(IncompleteEnum::inline(), r#"string"#)
}

#[derive(TS)]
#[ts(export, export_to = "top_level_type_override/")]
#[ts(type = "string")]
pub struct DataUrl {
    pub mime: String,
    pub contents: Vec<u8>,
}

#[test]
pub fn top_level_type_override_struct() {
    assert_eq!(DataUrl::inline(), r#"string"#)
}
