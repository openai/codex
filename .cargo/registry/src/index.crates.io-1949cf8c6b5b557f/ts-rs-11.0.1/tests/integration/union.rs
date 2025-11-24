#![allow(dead_code)]

use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "union/")]
enum SimpleEnum {
    #[ts(rename = "asdf")]
    A,
    B,
    C,
    r#D,
}

#[test]
fn test_empty() {
    #[derive(TS)]
    enum Empty {}

    assert_eq!(Empty::decl(), "type Empty = never;")
}

#[test]
fn test_simple_enum() {
    assert_eq!(
        SimpleEnum::decl(),
        r#"type SimpleEnum = "asdf" | "B" | "C" | "D";"#
    )
}
