use super::AdditionalProperties;
use super::JsonSchema;
use super::parse_tool_input_schema;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

// Tests in this section exercise normalization transforms that mutate badly
// formed JSON for consumption by the Responses API.

#[test]
fn parse_tool_input_schema_coerces_boolean_schemas() {
    // Example schema shape:
    // true
    //
    // Expected normalization behavior:
    // - JSON Schema boolean forms are coerced to `{ "type": "string" }`
    //   because the baseline enum model cannot represent boolean-schema
    //   semantics directly.
    let schema = parse_tool_input_schema(&serde_json::json!(true)).expect("parse schema");

    assert_eq!(schema, JsonSchema::String { description: None });
}

#[test]
fn parse_tool_input_schema_infers_object_shape_and_defaults_properties() {
    // Example schema shape:
    // {
    //   "properties": {
    //     "query": { "description": "search query" }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - `properties` implies an object schema when `type` is omitted.
    // - The child property keeps its description and defaults to a string type.
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
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": {
    //     "page": { "type": "integer" },
    //     "tags": { "type": "array" }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - `"integer"` is accepted by the baseline model through the legacy
    //   number/integer alias.
    // - Arrays missing `items` receive a permissive string `items` schema.
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
                ("page".to_string(), JsonSchema::Number { description: None }),
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
    // Example schema shape:
    // {
    //   "type": "object",
    //   "additionalProperties": {
    //     "required": ["value"],
    //     "properties": {
    //       "value": {
    //         "anyOf": [
    //           { "type": "string" },
    //           { "type": "number" }
    //         ]
    //       }
    //     }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - `additionalProperties` schema objects are recursively sanitized.
    // - The nested schema is normalized into the baseline object form.
    // - In the baseline model, the nested `anyOf` degrades to a plain string
    //   field because richer combiners are not preserved.
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

#[test]
fn parse_tool_input_schema_infers_object_shape_from_boolean_additional_properties_only() {
    // Example schema shape:
    // {
    //   "additionalProperties": false
    // }
    //
    // Expected normalization behavior:
    // - `additionalProperties` implies an object schema when `type` is omitted.
    // - The boolean `additionalProperties` setting is preserved.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "additionalProperties": false
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::Object {
            properties: BTreeMap::new(),
            required: None,
            additional_properties: Some(false.into()),
        }
    );
}

#[test]
fn parse_tool_input_schema_infers_number_from_numeric_keywords() {
    // Example schema shape:
    // {
    //   "minimum": 1
    // }
    //
    // Expected normalization behavior:
    // - Numeric constraint keywords imply a number schema when `type` is
    //   omitted.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "minimum": 1
    }))
    .expect("parse schema");

    assert_eq!(schema, JsonSchema::Number { description: None });
}

#[test]
fn parse_tool_input_schema_infers_number_from_multiple_of() {
    // Example schema shape:
    // {
    //   "multipleOf": 5
    // }
    //
    // Expected normalization behavior:
    // - `multipleOf` follows the same numeric-keyword inference path as
    //   `minimum` / `maximum`.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "multipleOf": 5
    }))
    .expect("parse schema");

    assert_eq!(schema, JsonSchema::Number { description: None });
}

#[test]
fn parse_tool_input_schema_infers_string_from_enum_const_and_format_keywords() {
    // Example schema shapes:
    // { "enum": ["fast", "safe"] }
    // { "const": "file" }
    // { "format": "date-time" }
    //
    // Expected normalization behavior:
    // - Each of these keywords implies a string schema when `type` is omitted.
    let enum_schema = parse_tool_input_schema(&serde_json::json!({
        "enum": ["fast", "safe"]
    }))
    .expect("parse enum schema");
    let const_schema = parse_tool_input_schema(&serde_json::json!({
        "const": "file"
    }))
    .expect("parse const schema");
    let format_schema = parse_tool_input_schema(&serde_json::json!({
        "format": "date-time"
    }))
    .expect("parse format schema");

    assert_eq!(enum_schema, JsonSchema::String { description: None });
    assert_eq!(const_schema, JsonSchema::String { description: None });
    assert_eq!(format_schema, JsonSchema::String { description: None });
}

#[test]
fn parse_tool_input_schema_defaults_empty_schema_to_string() {
    // Example schema shape:
    // {}
    //
    // Expected normalization behavior:
    // - With no structural hints at all, the baseline normalizer falls back to
    //   a permissive string schema.
    let schema = parse_tool_input_schema(&serde_json::json!({})).expect("parse schema");

    assert_eq!(schema, JsonSchema::String { description: None });
}

#[test]
fn parse_tool_input_schema_infers_array_from_prefix_items() {
    // Example schema shape:
    // {
    //   "prefixItems": [
    //     { "type": "string" }
    //   ]
    // }
    //
    // Expected normalization behavior:
    // - `prefixItems` implies an array schema when `type` is omitted.
    // - The baseline model still stores the normalized result as a regular
    //   array schema with string items.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "prefixItems": [
            {"type": "string"}
        ]
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::Array {
            items: Box::new(JsonSchema::String { description: None }),
            description: None,
        }
    );
}

#[test]
fn parse_tool_input_schema_preserves_boolean_additional_properties_on_inferred_object() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": {
    //     "metadata": {
    //       "additionalProperties": true
    //     }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - The nested `metadata` schema is inferred to be an object because it has
    //   `additionalProperties`.
    // - `additionalProperties: true` is preserved rather than rewritten.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "metadata": {
                "additionalProperties": true
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::Object {
            properties: BTreeMap::from([(
                "metadata".to_string(),
                JsonSchema::Object {
                    properties: BTreeMap::new(),
                    required: None,
                    additional_properties: Some(AdditionalProperties::Boolean(true)),
                },
            )]),
            required: None,
            additional_properties: None,
        }
    );
}

#[test]
fn parse_tool_input_schema_infers_object_shape_from_schema_additional_properties_only() {
    // Example schema shape:
    // {
    //   "additionalProperties": {
    //     "type": "string"
    //   }
    // }
    //
    // Expected normalization behavior:
    // - A schema-valued `additionalProperties` also implies an object schema
    //   when `type` is omitted.
    // - The nested schema is preserved as the object's
    //   `additionalProperties` definition.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "additionalProperties": {
            "type": "string"
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::Object {
            properties: BTreeMap::new(),
            required: None,
            additional_properties: Some(AdditionalProperties::Schema(Box::new(
                JsonSchema::String { description: None },
            ))),
        }
    );
}

// Schemas that should be preserved for Responses API compatibility rather than
// being rewritten into a different shape. These currently fail on the baseline
// normalizer and are the intended signal for the new JsonSchema work.

#[test]
fn parse_tool_input_schema_preserves_nested_nullable_type_union() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": {
    //     "nickname": {
    //       "type": ["string", "null"],
    //       "description": "Optional nickname"
    //     }
    //   },
    //   "required": ["nickname"],
    //   "additionalProperties": false
    // }
    //
    // Expected normalization behavior:
    // - The nested property keeps the explicit `["string", "null"]` union.
    // - The object-level `required` and `additionalProperties: false` stay intact.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "nickname": {
                "type": ["string", "null"],
                "description": "Optional nickname"
            }
        },
        "required": ["nickname"],
        "additionalProperties": false
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::Object {
            properties: BTreeMap::from([(
                "nickname".to_string(),
                serde_json::from_value(serde_json::json!({
                    "type": ["string", "null"],
                    "description": "Optional nickname"
                }))
                .expect("nested nullable schema"),
            )]),
            required: Some(vec!["nickname".to_string()]),
            additional_properties: Some(false.into()),
        }
    );
}

#[test]
fn parse_tool_input_schema_preserves_nested_any_of_property() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": {
    //     "query": {
    //       "anyOf": [
    //         { "type": "string" },
    //         { "type": "number" }
    //       ]
    //     }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - The nested `anyOf` is preserved rather than flattened into a single
    //   fallback type.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "query": {
                "anyOf": [
                    { "type": "string" },
                    { "type": "number" }
                ]
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::Object {
            properties: BTreeMap::from([(
                "query".to_string(),
                serde_json::from_value(serde_json::json!({
                    "anyOf": [
                        { "type": "string" },
                        { "type": "number" }
                    ]
                }))
                .expect("nested anyOf schema"),
            )]),
            required: None,
            additional_properties: None,
        }
    );
}
