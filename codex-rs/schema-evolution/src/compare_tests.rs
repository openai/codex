use super::*;
use crate::ViolationKind;
use crate::test_support::breakage;
use crate::test_support::compare;
use crate::test_support::request_schema;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn detects_method_removal() -> Result<()> {
    assert_eq!(
        compare(
            &request_schema(json!({ "type": "null" })),
            &json!({ "oneOf": [] }),
        )?,
        vec![crate::SchemaBreakage {
            kind: ViolationKind::MethodRemoved,
            method: "test/method".to_string(),
            path: "request".to_string(),
            before: json!(true),
            after: json!(false),
        }]
    );
    Ok(())
}

#[test]
fn compares_typed_arguments_with_the_full_request_schema() -> Result<()> {
    let mut base = request_schema(json!({ "type": "null" }));
    base["oneOf"][0]["required"] = json!(["id", "method"]);
    let current = request_schema(json!({ "type": "null" }));
    assert_eq!(
        compare(&base, &current)?,
        vec![breakage(
            ViolationKind::RequiredPropertyAdded,
            "params",
            json!(false),
            json!(true),
        )]
    );

    Ok(())
}

#[test]
fn retains_request_level_constraints_in_the_typed_envelope() -> Result<()> {
    let base = request_schema(json!({ "type": "null" }));
    let mut current = request_schema(json!({ "type": "null" }));
    current["oneOf"][0]["minProperties"] = json!(3);

    let mut expected = breakage(
        ViolationKind::ConstraintChanged,
        "request",
        json!({}),
        json!({ "minProperties": 3 }),
    );
    expected.method = "*".to_string();
    assert_eq!(compare(&base, &current)?, vec![expected]);
    Ok(())
}
