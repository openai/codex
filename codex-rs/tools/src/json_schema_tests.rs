use super::AdditionalProperties;
use super::JsonSchema;
use super::JsonSchemaPrimitiveType;
use super::JsonSchemaType;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn parse_tool_input_schema_coerces_boolean_schemas() {
    let schema = super::parse_tool_input_schema(&serde_json::json!(true)).expect("parse schema");

    assert_eq!(schema, JsonSchema::string(/*description*/ None));
}

#[test]
fn parse_tool_input_schema_infers_object_shape_and_defaults_properties() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
        "properties": {
            "query": {"description": "search query"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "query".to_string(),
                JsonSchema::string(Some("search query".to_string())),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_preserves_integer_and_defaults_array_items() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "page": {"type": "integer"},
            "tags": {"type": "array"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([
                (
                    "page".to_string(),
                    JsonSchema::integer(/*description*/ None),
                ),
                (
                    "tags".to_string(),
                    JsonSchema::array(
                        JsonSchema::string(/*description*/ None),
                        /*description*/ None,
                    )
                ),
            ]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_sanitizes_additional_properties_schema() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
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
        JsonSchema::object(
            BTreeMap::new(),
            /*required*/ None,
            Some(AdditionalProperties::Schema(Box::new(JsonSchema::object(
                BTreeMap::from([(
                    "value".to_string(),
                    JsonSchema::any_of(
                        vec![
                            JsonSchema::string(/*description*/ None),
                            JsonSchema::number(/*description*/ None),
                        ],
                        /*description*/ None,
                    ),
                )]),
                Some(vec!["value".to_string()]),
                /*additional_properties*/ None,
            ))))
        )
    );
}

#[test]
fn parse_tool_input_schema_infers_object_shape_from_boolean_additional_properties_only() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
        "additionalProperties": false
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::new(),
            /*required*/ None,
            Some(false.into())
        )
    );
}

#[test]
fn parse_tool_input_schema_infers_number_from_numeric_keywords() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
        "minimum": 1
    }))
    .expect("parse schema");

    assert_eq!(schema, JsonSchema::number(/*description*/ None));
}

#[test]
fn parse_tool_input_schema_infers_number_from_multiple_of() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
        "multipleOf": 5
    }))
    .expect("parse schema");

    assert_eq!(schema, JsonSchema::number(/*description*/ None));
}

#[test]
fn parse_tool_input_schema_infers_string_from_enum_const_and_format_keywords() {
    let enum_schema = super::parse_tool_input_schema(&serde_json::json!({
        "enum": ["fast", "safe"]
    }))
    .expect("parse enum schema");
    let const_schema = super::parse_tool_input_schema(&serde_json::json!({
        "const": "file"
    }))
    .expect("parse const schema");
    let format_schema = super::parse_tool_input_schema(&serde_json::json!({
        "format": "date-time"
    }))
    .expect("parse format schema");

    assert_eq!(
        enum_schema,
        JsonSchema::string_enum(
            vec![serde_json::json!("fast"), serde_json::json!("safe")],
            /*description*/ None,
        )
    );
    assert_eq!(
        const_schema,
        JsonSchema::string_enum(vec![serde_json::json!("file")], /*description*/ None)
    );
    assert_eq!(format_schema, JsonSchema::string(/*description*/ None));
}

#[test]
fn parse_tool_input_schema_defaults_empty_schema_to_string() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({})).expect("parse schema");

    assert_eq!(schema, JsonSchema::string(/*description*/ None));
}

#[test]
fn parse_tool_input_schema_infers_array_from_prefix_items() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
        "prefixItems": [
            {"type": "string"}
        ]
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::array(
            JsonSchema::string(/*description*/ None),
            /*description*/ None,
        )
    );
}

#[test]
fn parse_tool_input_schema_preserves_boolean_additional_properties_on_inferred_object() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
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
        JsonSchema::object(
            BTreeMap::from([(
                "metadata".to_string(),
                JsonSchema::object(BTreeMap::new(), /*required*/ None, Some(true.into())),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_infers_object_shape_from_schema_additional_properties_only() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
        "additionalProperties": {
            "type": "string"
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::new(),
            /*required*/ None,
            Some(JsonSchema::string(/*description*/ None).into())
        )
    );
}

#[test]
fn parse_tool_input_schema_preserves_nested_nullable_any_of_shape() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "open": {
                "anyOf": [
                    {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "ref_id": {"type": "string"},
                                "lineno": {"anyOf": [{"type": "integer"}, {"type": "null"}]}
                            },
                            "required": ["ref_id"],
                            "additionalProperties": false
                        }
                    },
                    {"type": "null"}
                ]
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "open".to_string(),
                JsonSchema::any_of(
                    vec![
                        JsonSchema::array(
                            JsonSchema::object(
                                BTreeMap::from([
                                    (
                                        "lineno".to_string(),
                                        JsonSchema::any_of(
                                            vec![
                                                JsonSchema::integer(/*description*/ None),
                                                JsonSchema::null(/*description*/ None),
                                            ],
                                            /*description*/ None,
                                        ),
                                    ),
                                    (
                                        "ref_id".to_string(),
                                        JsonSchema::string(/*description*/ None),
                                    ),
                                ]),
                                Some(vec!["ref_id".to_string()]),
                                Some(false.into()),
                            ),
                            /*description*/ None,
                        ),
                        JsonSchema::null(/*description*/ None),
                    ],
                    /*description*/ None,
                ),
            ),]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
#[ignore = "Expected to pass after the new JsonSchema preserves nullable type unions"]
fn parse_tool_input_schema_preserves_nested_nullable_type_union() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
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
        JsonSchema::object(
            BTreeMap::from([(
                "nickname".to_string(),
                JsonSchema {
                    schema_type: Some(JsonSchemaType::Multiple(vec![
                        JsonSchemaPrimitiveType::String,
                        JsonSchemaPrimitiveType::Null,
                    ])),
                    description: Some("Optional nickname".to_string()),
                    ..Default::default()
                },
            )]),
            Some(vec!["nickname".to_string()]),
            Some(false.into()),
        )
    );
}

#[test]
#[ignore = "Expected to pass after the new JsonSchema preserves nested anyOf schemas"]
fn parse_tool_input_schema_preserves_nested_any_of_property() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
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
        JsonSchema::object(
            BTreeMap::from([(
                "query".to_string(),
                JsonSchema::any_of(
                    vec![
                        JsonSchema::string(/*description*/ None),
                        JsonSchema::number(/*description*/ None),
                    ],
                    /*description*/ None,
                ),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_preserves_type_unions_without_rewriting_to_any_of() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
        "type": ["string", "null"],
        "description": "optional string"
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Multiple(vec![
                JsonSchemaPrimitiveType::String,
                JsonSchemaPrimitiveType::Null,
            ])),
            description: Some("optional string".to_string()),
            ..Default::default()
        }
    );
}

#[test]
fn parse_tool_input_schema_preserves_explicit_enum_type_union() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
        "type": ["string", "null"],
        "enum": ["short", "medium", "long"],
        "description": "optional response length"
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Multiple(vec![
                JsonSchemaPrimitiveType::String,
                JsonSchemaPrimitiveType::Null,
            ])),
            description: Some("optional response length".to_string()),
            enum_values: Some(vec![
                serde_json::json!("short"),
                serde_json::json!("medium"),
                serde_json::json!("long"),
            ]),
            ..Default::default()
        }
    );
}

#[test]
fn parse_tool_input_schema_preserves_string_enum_constraints() {
    let schema = super::parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "response_length": {
                "type": "enum",
                "enum": ["short", "medium", "long"]
            },
            "kind": {
                "type": "const",
                "const": "tagged"
            },
            "scope": {
                "type": "enum",
                "enum": ["one", "two"]
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([
                (
                    "kind".to_string(),
                    JsonSchema::string_enum(
                        vec![serde_json::json!("tagged")],
                        /*description*/ None,
                    ),
                ),
                (
                    "response_length".to_string(),
                    JsonSchema::string_enum(
                        vec![
                            serde_json::json!("short"),
                            serde_json::json!("medium"),
                            serde_json::json!("long"),
                        ],
                        /*description*/ None,
                    ),
                ),
                (
                    "scope".to_string(),
                    JsonSchema::string_enum(
                        vec![serde_json::json!("one"), serde_json::json!("two")],
                        /*description*/ None,
                    ),
                ),
            ]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}
