use crate::ViolationKind;
use crate::test_support::breakage;
use crate::test_support::compare;
use crate::test_support::request_schema;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn detects_enum_type_and_bound_narrowing() -> anyhow::Result<()> {
    assert_eq!(
        compare(
            &request_schema(json!({ "enum": ["a", "b"], "type": "string" })),
            &request_schema(json!({ "enum": ["a"], "type": "string" })),
        )?,
        vec![breakage(
            ViolationKind::EnumNarrowed,
            "params",
            json!(["a", "b"]),
            json!(["a"]),
        )]
    );
    assert_eq!(
        compare(
            &request_schema(json!({ "type": "number" })),
            &request_schema(json!({ "type": "integer" })),
        )?,
        vec![breakage(
            ViolationKind::TypeNarrowed,
            "params",
            json!("number"),
            json!("integer"),
        )]
    );
    assert_eq!(
        compare(
            &request_schema(json!({ "type": ["string", "null"] })),
            &request_schema(json!({ "type": "string" })),
        )?,
        vec![breakage(
            ViolationKind::TypeNarrowed,
            "params",
            json!(["null", "string"]),
            json!("string"),
        )]
    );
    assert_eq!(
        compare(
            &request_schema(json!({ "minLength": 1, "type": "string" })),
            &request_schema(json!({ "minLength": 2, "type": "string" })),
        )?,
        vec![breakage(
            ViolationKind::ConstraintChanged,
            "params",
            json!({ "minLength": 1 }),
            json!({ "minLength": 2 }),
        )]
    );
    Ok(())
}

#[test]
fn allows_enum_type_and_bound_widening() -> anyhow::Result<()> {
    let base = request_schema(json!({ "enum": ["a"], "minLength": 2, "type": "string" }));
    let current =
        request_schema(json!({ "enum": ["a", "b"], "minLength": 1, "type": ["string", "null"] }));
    assert_eq!(compare(&base, &current)?, vec![]);
    Ok(())
}

#[test]
fn compares_large_integer_bounds_without_losing_precision() -> anyhow::Result<()> {
    let base = request_schema(json!({
        "minimum": 9_007_199_254_740_992_u64,
        "type": "integer"
    }));
    let current = request_schema(json!({
        "minimum": 9_007_199_254_740_993_u64,
        "type": "integer"
    }));
    assert_eq!(
        compare(&base, &current)?,
        vec![breakage(
            ViolationKind::ConstraintChanged,
            "params",
            json!({ "minimum": 9_007_199_254_740_992_u64 }),
            json!({ "minimum": 9_007_199_254_740_993_u64 }),
        )]
    );
    Ok(())
}

#[test]
fn intersects_enum_and_const_constraints() -> anyhow::Result<()> {
    let base = request_schema(json!({ "enum": ["a", "b"], "type": "string" }));
    let current = request_schema(json!({
        "const": "a",
        "enum": ["a", "b"],
        "type": "string"
    }));
    assert_eq!(
        compare(&base, &current)?,
        vec![breakage(
            ViolationKind::EnumNarrowed,
            "params",
            json!(["a", "b"]),
            json!(["a"]),
        )]
    );
    Ok(())
}
