use super::JsonSchema;
use super::parse_tool_input_schema;
use super::validate_structured_outputs_schema;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

// This file codifies the Structured Outputs invariants that matter for the MCP
// schema regression in PR #18159.
//
// Source of truth:
// https://developers.openai.com/api/docs/guides/structured-outputs#supported-schemas
//
// The doc page calls out several constraints that are relevant to the object
// schemas we expose through the Responses API:
// - the root schema must be an object and must not be `anyOf`
// - all object fields / function parameters must appear in `required`
// - every object must set `additionalProperties: false`
//
// The flattening bug in this PR was specifically about `$ref` and
// single-variant combiner wrappers causing nested object parameters like
// `start` / `end` to collapse to `string`. The tests below intentionally use
// Structured Outputs-compliant inputs so we can assert not only that the object
// shape survives, but also that the surviving shape still lives inside the
// Responses API subset documented above.

#[test]
fn parse_tool_input_schema_keeps_local_ref_objects_inside_structured_outputs_subset() {
    // This mirrors the Outlook Calendar `create_event.start` / `end` shape we
    // care about, but does so with the exact Structured Outputs object
    // invariants from:
    // https://developers.openai.com/api/docs/guides/structured-outputs#supported-schemas
    //
    // We want to prove that resolving a local `$ref` no longer collapses the
    // nested object to `string`, while also preserving:
    // - root object shape
    // - `additionalProperties: false`
    // - `required` coverage for every field
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "start": { "$ref": "#/$defs/date_time_zone" }
        },
        "required": ["start"],
        "additionalProperties": false,
        "$defs": {
            "date_time_zone": {
                "type": "object",
                "properties": {
                    "dateTime": {
                        "type": "string",
                        "description": "RFC3339 timestamp"
                    },
                    "timeZone": {
                        "type": "string",
                        "description": "IANA time zone"
                    }
                },
                "required": ["dateTime", "timeZone"],
                "additionalProperties": false
            }
        }
    }))
    .expect("parse schema");

    validate_structured_outputs_schema(&schema).expect("schema should stay in supported subset");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "start".to_string(),
                JsonSchema::object(
                    BTreeMap::from([
                        (
                            "dateTime".to_string(),
                            JsonSchema::string(Some("RFC3339 timestamp".to_string())),
                        ),
                        (
                            "timeZone".to_string(),
                            JsonSchema::string(Some("IANA time zone".to_string())),
                        ),
                    ]),
                    Some(vec!["dateTime".to_string(), "timeZone".to_string()]),
                    Some(false.into()),
                ),
            )]),
            Some(vec!["start".to_string()]),
            Some(false.into()),
        )
    );
}

#[test]
fn parse_tool_input_schema_keeps_single_variant_combiner_objects_inside_structured_outputs_subset()
{
    // The docs allow nested objects, but those nested objects still need the
    // same strict object invariants:
    // https://developers.openai.com/api/docs/guides/structured-outputs#supported-schemas
    //
    // This test covers the second half of the regression: a single-variant
    // `allOf` wrapper around the `DateTimeTimeZone` object must unwrap back to
    // an object without losing the Structured Outputs constraints that make the
    // schema acceptable to the Responses API.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "end": {
                "allOf": [{
                    "type": "object",
                    "properties": {
                        "dateTime": { "type": "string" },
                        "timeZone": { "type": "string" }
                    },
                    "required": ["dateTime", "timeZone"],
                    "additionalProperties": false
                }]
            }
        },
        "required": ["end"],
        "additionalProperties": false
    }))
    .expect("parse schema");

    validate_structured_outputs_schema(&schema).expect("schema should stay in supported subset");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "end".to_string(),
                JsonSchema::object(
                    BTreeMap::from([
                        (
                            "dateTime".to_string(),
                            JsonSchema::string(/*description*/ None),
                        ),
                        (
                            "timeZone".to_string(),
                            JsonSchema::string(/*description*/ None),
                        ),
                    ]),
                    Some(vec!["dateTime".to_string(), "timeZone".to_string()]),
                    Some(false.into()),
                ),
            )]),
            Some(vec!["end".to_string()]),
            Some(false.into()),
        )
    );
}
