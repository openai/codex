use crate::ViolationKind;
use crate::test_support::breakage;
use crate::test_support::compare;
use crate::test_support::request_schema;
use crate::test_support::sorted;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn detects_enum_type_and_bound_narrowing() -> anyhow::Result<()> {
    let base = request_schema(json!({
        "properties": {
            "choice": { "enum": ["a", "b"], "type": "string" },
            "length": { "minLength": 1, "type": "string" },
            "numeric": { "type": "number" },
            "value": { "type": ["string", "null"] }
        },
        "type": "object"
    }));
    let current = request_schema(json!({
        "properties": {
            "choice": { "enum": ["a"], "type": "string" },
            "length": { "minLength": 2, "type": "string" },
            "numeric": { "type": "integer" },
            "value": { "type": "string" }
        },
        "type": "object"
    }));

    assert_eq!(
        compare(&base, &current)?,
        sorted(vec![
            breakage(
                ViolationKind::EnumNarrowed,
                "params.choice",
                json!(["a", "b"]),
                json!(["a"]),
            ),
            breakage(
                ViolationKind::ConstraintChanged,
                "params.length",
                json!({ "minLength": 1 }),
                json!({ "minLength": 2 }),
            ),
            breakage(
                ViolationKind::TypeNarrowed,
                "params.numeric",
                json!("number"),
                json!("integer"),
            ),
            breakage(
                ViolationKind::TypeNarrowed,
                "params.value",
                json!(["null", "string"]),
                json!("string"),
            ),
        ])
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
