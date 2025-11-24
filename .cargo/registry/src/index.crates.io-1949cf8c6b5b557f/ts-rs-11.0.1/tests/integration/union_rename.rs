#![allow(dead_code)]

use ts_rs::TS;

#[derive(TS)]
#[ts(export, export_to = "union_rename/")]
#[ts(rename_all = "lowercase", rename = "SimpleEnum")]
enum RenamedEnum {
    #[ts(rename = "ASDF")]
    A,
    #[ts(rename = &"BB")]
    B,
    #[ts(rename = "C".repeat(2))]
    C,
}

#[test]
fn test_simple_enum() {
    assert_eq!(
        RenamedEnum::decl(),
        r#"type SimpleEnum = "ASDF" | "BB" | "CC";"#
    )
}

#[derive(TS)]
#[ts(export, export_to = "union_rename/")]
#[ts(rename = format!("{}With{}", "Renamed", "StringExpression"))]
enum WithStringExpression {
    A,
    B,
    C,
}

#[test]
fn test_rename_with_string_expression() {
    assert_eq!(
        WithStringExpression::decl(),
        r#"type RenamedWithStringExpression = "A" | "B" | "C";"#
    )
}

#[derive(TS)]
#[ts(export, export_to = "union_rename/")]
#[ts(rename = &"RenamedWithStrExpression")]
enum WithStrExpression {
    A,
    B,
    C,
}

#[test]
fn test_rename_with_str_expression() {
    assert_eq!(
        WithStrExpression::decl(),
        r#"type RenamedWithStrExpression = "A" | "B" | "C";"#
    )
}

#[derive(TS)]
#[ts(export, export_to = "union_rename/")]
#[ts(rename = format!("i_am_inside_module_{}", module_path!().rsplit_once("::").unwrap().1))]
enum RenameUsingModuleName {
    A,
    B,
    C,
}

#[test]
fn test_rename_using_module_name() {
    assert_eq!(
        RenameUsingModuleName::decl(),
        r#"type i_am_inside_module_union_rename = "A" | "B" | "C";"#
    )
}
