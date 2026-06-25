use super::*;
use crate::ViolationKind;
use crate::test_support::breakage;
use crate::test_support::compare;
use crate::test_support::request_schema;
use crate::test_support::sorted;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn detects_property_required_and_additional_property_narrowing() -> Result<()> {
    let base = request_schema(json!({
        "properties": {
            "removed": { "type": "string" },
            "requiredLater": { "type": "string" }
        },
        "type": "object"
    }));
    let current = request_schema(json!({
        "additionalProperties": false,
        "properties": { "requiredLater": { "type": "string" } },
        "required": ["requiredLater"],
        "type": "object"
    }));

    assert_eq!(
        compare(&base, &current)?,
        sorted(vec![
            breakage(
                ViolationKind::AdditionalPropertiesNarrowed,
                "params.*",
                json!(true),
                json!(false),
            ),
            breakage(
                ViolationKind::PropertyRemoved,
                "params.removed",
                json!(true),
                json!(false),
            ),
            breakage(
                ViolationKind::RequiredPropertyAdded,
                "params.requiredLater",
                json!(false),
                json!(true),
            ),
        ])
    );
    Ok(())
}

#[test]
fn allows_object_widening() -> Result<()> {
    let base = request_schema(json!({
        "additionalProperties": false,
        "properties": { "value": { "type": "string" } },
        "required": ["value"],
        "type": "object"
    }));
    let current = request_schema(json!({
        "properties": {
            "newOptional": { "type": "boolean" },
            "value": { "type": "string" }
        },
        "type": "object"
    }));
    assert_eq!(compare(&base, &current)?, vec![]);
    Ok(())
}

#[test]
fn reports_each_path_that_uses_a_narrowed_shared_definition() -> Result<()> {
    let mut base = request_schema(json!({
        "properties": {
            "first": { "$ref": "#/definitions/Shared" },
            "second": { "$ref": "#/definitions/Shared" }
        },
        "type": "object"
    }));
    base["definitions"] = json!({ "Shared": { "type": "number" } });
    let mut current = base.clone();
    current["definitions"]["Shared"]["type"] = json!("integer");

    assert_eq!(
        compare(&base, &current)?,
        sorted(vec![
            breakage(
                ViolationKind::TypeNarrowed,
                "params.first",
                json!("number"),
                json!("integer"),
            ),
            breakage(
                ViolationKind::TypeNarrowed,
                "params.second",
                json!("number"),
                json!("integer"),
            ),
        ])
    );
    Ok(())
}
