use super::AdditionalProperties;
use super::JsonSchema;
use super::parse_tool_input_schema;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn parse_tool_input_schema_coerces_boolean_schemas() {
    let schema = parse_tool_input_schema(&serde_json::json!(true)).expect("parse schema");

    assert_eq!(schema, JsonSchema::String { description: None });
}

#[test]
fn parse_tool_input_schema_infers_object_shape_and_defaults_properties() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "properties": {
            "query": {"description": "search query"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::Object {
            properties: BTreeMap::from([(
                "query".to_string(),
                JsonSchema::String {
                    description: Some("search query".to_string()),
                },
            )]),
            required: None,
            additional_properties: None,
        }
    );
}

#[test]
fn parse_tool_input_schema_normalizes_integer_and_missing_array_items() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "page": {"type": "integer"},
            "tags": {"type": "array"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::Object {
            properties: BTreeMap::from([
                ("page".to_string(), JsonSchema::Number { description: None },),
                (
                    "tags".to_string(),
                    JsonSchema::Array {
                        items: Box::new(JsonSchema::String { description: None }),
                        description: None,
                    },
                ),
            ]),
            required: None,
            additional_properties: None,
        }
    );
}

#[test]
fn parse_tool_input_schema_sanitizes_additional_properties_schema() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "additionalProperties": {
            "required": ["value"],
            "properties": {
                "value": {"anyOf": [{"type": "string"}, {"type": "number"}]}
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::Object {
            properties: BTreeMap::new(),
            required: None,
            additional_properties: Some(AdditionalProperties::Schema(Box::new(
                JsonSchema::Object {
                    properties: BTreeMap::from([(
                        "value".to_string(),
                        JsonSchema::String { description: None },
                    )]),
                    required: Some(vec!["value".to_string()]),
                    additional_properties: None,
                },
            ))),
        }
    );
}
