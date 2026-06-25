use crate::ViolationKind;
use crate::test_support::breakage;
use crate::test_support::compare;
use crate::test_support::request_schema;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn detects_removed_union_variants_but_allows_widening() -> anyhow::Result<()> {
    let base = request_schema(json!({
        "anyOf": [{ "enum": ["a"] }, { "type": "null" }]
    }));
    let current = request_schema(json!({ "anyOf": [{ "enum": ["a", "b"] }] }));
    assert_eq!(
        compare(&base, &current)?,
        vec![breakage(
            ViolationKind::UnionVariantRemoved,
            "params",
            json!("type=\"null\""),
            json!(null),
        )]
    );

    let base = request_schema(json!({ "type": "string" }));
    let current = request_schema(json!({
        "anyOf": [{ "type": "string" }, { "type": "null" }]
    }));
    assert_eq!(compare(&base, &current)?, vec![]);
    Ok(())
}

#[test]
fn detects_new_overlap_in_one_of() -> anyhow::Result<()> {
    let base = request_schema(json!({
        "oneOf": [{ "enum": ["a"] }, { "enum": ["b"] }]
    }));
    let current = request_schema(json!({
        "oneOf": [{ "enum": ["a", "b"] }, { "enum": ["b", "c"] }]
    }));
    let violations = compare(&base, &current)?;
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].kind, ViolationKind::ConstraintChanged);
    assert_eq!(violations[0].path, "params");
    Ok(())
}

#[test]
fn compares_recursive_union_refs_without_reentering_forever() -> anyhow::Result<()> {
    let mut base = request_schema(json!({ "$ref": "#/definitions/Params" }));
    base["definitions"] = json!({
        "Params": {
            "properties": {
                "child": { "anyOf": [{ "$ref": "#/definitions/Params" }, { "type": "null" }] },
                "value": { "type": "number" }
            },
            "type": "object"
        }
    });
    let mut current = base.clone();
    current["definitions"]["Params"]["properties"]["value"]["type"] = json!("integer");

    assert_eq!(
        compare(&base, &current)?,
        vec![breakage(
            ViolationKind::TypeNarrowed,
            "params.value",
            json!("number"),
            json!("integer"),
        )]
    );
    Ok(())
}

#[test]
fn one_of_equivalence_preserves_duplicate_branch_multiplicity() -> anyhow::Result<()> {
    let base = request_schema(json!({
        "oneOf": [{ "enum": ["a"] }, { "enum": ["a"] }, { "enum": ["b"] }]
    }));
    let current = request_schema(json!({
        "oneOf": [{ "enum": ["a"] }, { "enum": ["b"] }, { "enum": ["b"] }]
    }));

    let violations = compare(&base, &current)?;
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].kind, ViolationKind::ConstraintChanged);
    assert_eq!(violations[0].path, "params");
    Ok(())
}
