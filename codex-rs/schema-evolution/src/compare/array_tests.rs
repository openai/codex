use crate::ViolationKind;
use crate::test_support::breakage;
use crate::test_support::compare;
use crate::test_support::request_schema;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn walks_homogeneous_and_tuple_items() -> anyhow::Result<()> {
    let base = request_schema(json!({
        "items": [{ "type": "string" }, { "type": "number" }],
        "type": "array"
    }));
    let current = request_schema(json!({
        "items": [{ "type": "string" }, { "type": "integer" }],
        "type": "array"
    }));
    assert_eq!(
        compare(&base, &current)?,
        vec![breakage(
            ViolationKind::TypeNarrowed,
            "params[1]",
            json!("number"),
            json!("integer"),
        )]
    );

    let base = request_schema(json!({ "items": { "type": "number" }, "type": "array" }));
    let current = request_schema(json!({ "items": { "type": "integer" }, "type": "array" }));
    assert_eq!(
        compare(&base, &current)?,
        vec![breakage(
            ViolationKind::TypeNarrowed,
            "params[]",
            json!("number"),
            json!("integer"),
        )]
    );
    Ok(())
}

#[test]
fn detects_shortening_a_closed_tuple() -> anyhow::Result<()> {
    let base = request_schema(json!({
        "additionalItems": false,
        "items": [{ "type": "string" }, { "type": "number" }],
        "type": "array"
    }));
    let current = request_schema(json!({
        "additionalItems": false,
        "items": [{ "type": "string" }],
        "type": "array"
    }));
    assert_eq!(
        compare(&base, &current)?,
        vec![breakage(
            ViolationKind::ConstraintChanged,
            "params[1]",
            json!({ "type": "number" }),
            json!(false),
        )]
    );
    Ok(())
}
