use super::AdditionalProperties;
use super::JsonSchema;
use super::JsonSchemaPrimitiveType;
use super::JsonSchemaType;
use super::parse_tool_input_schema;
use super::parse_tool_input_schema_without_compaction;
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

    assert_eq!(schema, JsonSchema::string(/*description*/ None));
}

#[test]
fn json_schema_serializes_encrypted_marker() {
    let schema = JsonSchema::string(Some("Secret value".to_string())).with_encrypted();

    assert_eq!(
        serde_json::to_value(schema).expect("serialize schema"),
        serde_json::json!({
            "type": "string",
            "description": "Secret value",
            "encrypted": true,
        })
    );
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
    // - The child property has no recognized schema hints, so it is coerced to
    //   an empty permissive schema.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "properties": {
            "query": {"description": "search query"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([("query".to_string(), JsonSchema::default())]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_coerces_unrecognized_object_schema_to_empty_schema() {
    // Example schema shape:
    // {
    //   "description": "Ticket identifier",
    //   "title": "Ticket ID"
    // }
    //
    // Expected normalization behavior:
    // - Object schemas with no recognized schema hints are treated as
    //   malformed and coerced to the empty permissive schema.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "description": "Ticket identifier",
        "title": "Ticket ID"
    }))
    .expect("parse schema");

    assert_eq!(schema, JsonSchema::default());
}

#[test]
fn parse_tool_input_schema_preserves_integer_and_defaults_array_items() {
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
    // - `"integer"` is preserved distinctly from `"number"`.
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
    // - The nested schema is normalized into the current object/anyOf form.
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
        JsonSchema::object(BTreeMap::new(), /*required*/ None, Some(false.into()))
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

    assert_eq!(schema, JsonSchema::number(/*description*/ None));
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

    assert_eq!(schema, JsonSchema::number(/*description*/ None));
}

#[test]
fn parse_tool_input_schema_infers_string_from_enum_const_and_format_keywords() {
    // Example schema shapes:
    // { "enum": ["fast", "safe"] }
    // { "const": "file" }
    // { "format": "date-time" }
    //
    // Expected normalization behavior:
    // - `enum` and `const` normalize into explicit string-enum schemas.
    // - `format` still falls back to a plain string schema.
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
fn parse_tool_input_schema_preserves_empty_schema() {
    // Example schema shape:
    // {}
    //
    // Expected normalization behavior:
    // - An empty JSON Schema is already a valid permissive schema, so it stays
    //   empty rather than being rewritten as an object schema.
    let schema = parse_tool_input_schema(&serde_json::json!({})).expect("parse schema");

    assert_eq!(schema, JsonSchema::default());
}

#[test]
fn parse_tool_input_schema_preserves_nested_empty_schema() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": {
    //     "metadata": {
    //       "properties": {
    //         "extra": {}
    //       }
    //     }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - The sanitizer recurses through nested object properties.
    // - The innermost `extra` field is an empty JSON Schema and stays empty.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "metadata": {
                "properties": {
                    "extra": {}
                }
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "metadata".to_string(),
                JsonSchema::object(
                    BTreeMap::from([("extra".to_string(), JsonSchema::default())]),
                    /*required*/ None,
                    /*additional_properties*/ None,
                )
            )]),
            /*required*/ None,
            /*additional_properties*/ None,
        )
    );
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
    // - The normalized result is stored as a regular array schema with string
    //   items.
    let schema = parse_tool_input_schema(&serde_json::json!({
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
        JsonSchema::object(
            BTreeMap::new(),
            /*required*/ None,
            Some(JsonSchema::string(/*description*/ None).into())
        )
    );
}

#[test]
fn parse_tool_input_schema_rewrites_const_to_single_value_enum() {
    // Example schema shape:
    // {
    //   "const": "tagged"
    // }
    //
    // Expected normalization behavior:
    // - `const` is rewritten through the sanitizer's `map.remove("const")`
    //   path into an equivalent single-value string enum schema.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "const": "tagged"
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::string_enum(vec![serde_json::json!("tagged")], /*description*/ None)
    );
}

#[test]
fn parse_tool_input_schema_rejects_singleton_null_type() {
    let err = parse_tool_input_schema(&serde_json::json!({
        "type": "null"
    }))
    .expect_err("singleton null should be rejected");

    assert!(
        err.to_string()
            .contains("tool input schema must not be a singleton null type"),
        "unexpected error: {err}"
    );
}

#[test]
fn parse_tool_input_schema_fills_default_properties_for_nullable_object_union() {
    // Example schema shape:
    // {
    //   "type": ["object", "null"]
    // }
    //
    // Expected normalization behavior:
    // - The full union is preserved.
    // - Object members of the union still receive default `properties`.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": ["object", "null"]
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Multiple(vec![
                JsonSchemaPrimitiveType::Object,
                JsonSchemaPrimitiveType::Null,
            ])),
            properties: Some(BTreeMap::new()),
            ..Default::default()
        }
    );
}

#[test]
fn parse_tool_input_schema_fills_default_items_for_nullable_array_union() {
    // Example schema shape:
    // {
    //   "type": ["array", "null"]
    // }
    //
    // Expected normalization behavior:
    // - The full union is preserved.
    // - Array members of the union still receive default `items`.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": ["array", "null"]
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Multiple(vec![
                JsonSchemaPrimitiveType::Array,
                JsonSchemaPrimitiveType::Null,
            ])),
            items: Some(Box::new(JsonSchema::string(/*description*/ None))),
            ..Default::default()
        }
    );
}

// Schemas that should be preserved for Responses API compatibility rather than
// being rewritten into a different shape.

#[test]
fn parse_tool_input_schema_preserves_nested_nullable_any_of_shape() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": {
    //     "open": {
    //       "anyOf": [
    //         {
    //           "type": "array",
    //           "items": {
    //             "type": "object",
    //             "properties": {
    //               "ref_id": { "type": "string" },
    //               "lineno": { "anyOf": [{ "type": "integer" }, { "type": "null" }] }
    //             },
    //             "required": ["ref_id"],
    //             "additionalProperties": false
    //           }
    //         },
    //         { "type": "null" }
    //       ]
    //     }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - Nested nullable `anyOf` shapes are preserved all the way down.
    let schema = parse_tool_input_schema(&serde_json::json!({
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
    // Example schema shape:
    // {
    //   "type": ["string", "null"],
    //   "description": "optional string"
    // }
    //
    // Expected normalization behavior:
    // - Explicit type unions are preserved as unions rather than rewritten to
    //   `anyOf`.
    let schema = parse_tool_input_schema(&serde_json::json!({
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
    // Example schema shape:
    // {
    //   "type": ["string", "null"],
    //   "enum": ["short", "medium", "long"],
    //   "description": "optional response length"
    // }
    //
    // Expected normalization behavior:
    // - The explicit string/null union is preserved alongside the enum values.
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

fn many_string_properties(count: usize) -> serde_json::Map<String, serde_json::Value> {
    (0..count)
        .map(|index| {
            (
                format!("field_{index:03}"),
                serde_json::json!({ "type": "string" }),
            )
        })
        .collect()
}

#[test]
fn parse_large_tool_input_schema_compacts_descriptions_only_on_default_path() {
    let input_schema = serde_json::json!({
        "type": "object",
        "description": "x".repeat(4_500),
        "properties": {
            "metadata": {
                "$ref": "#/$defs/metadata"
            }
        },
        "$defs": {
            "metadata": {
                "type": "string",
                "description": "Metadata value"
            }
        }
    });
    let schema = parse_tool_input_schema(&input_schema).expect("parse schema");

    assert_eq!(
        serde_json::to_value(schema).expect("serialize schema"),
        serde_json::json!({
            "type": "object",
            "properties": {
                "metadata": {
                    "$ref": "#/$defs/metadata"
                }
            },
            "$defs": {
                "metadata": {
                    "type": "string"
                }
            }
        })
    );

    let schema = parse_tool_input_schema_without_compaction(&input_schema).expect("parse schema");
    assert_eq!(
        serde_json::to_value(schema).expect("serialize schema"),
        serde_json::json!({
            "type": "object",
            "description": "x".repeat(4_500),
            "properties": {
                "metadata": {
                    "$ref": "#/$defs/metadata"
                }
            },
            "$defs": {
                "metadata": {
                    "type": "string",
                    "description": "Metadata value"
                }
            }
        })
    );
}

#[test]
fn parse_large_tool_input_schema_ignores_dropped_metadata_for_budget() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "event": {
                "type": "object",
                "title": "Calendar event",
                "properties": {
                    "recurrence": {
                        "type": "object",
                        "examples": [
                            {
                                "payload": "x".repeat(4_500)
                            }
                        ],
                        "properties": {
                            "pattern": {
                                "type": "string",
                                "title": "Recurrence pattern"
                            }
                        }
                    }
                }
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        serde_json::to_value(schema).expect("serialize schema"),
        serde_json::json!({
            "type": "object",
            "properties": {
                "event": {
                    "type": "object",
                    "properties": {
                        "recurrence": {
                            "type": "object",
                            "properties": {
                                "pattern": {
                                    "type": "string"
                                }
                            }
                        }
                    }
                }
            }
        })
    );
}

#[test]
fn parse_large_tool_input_schema_stops_after_dropping_root_definitions_when_under_budget() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "description": "x".repeat(4_500),
        "properties": {
            "event": {
                "type": "object",
                "description": "Calendar event",
                "properties": {
                    "recurrence": {
                        "type": "object",
                        "description": "Recurrence settings",
                        "properties": {
                            "pattern": {
                                "type": "string",
                                "description": "Recurrence pattern"
                            }
                        }
                    }
                }
            },
            "metadata": {
                "$ref": "#/$defs/metadata"
            }
        },
        "$defs": {
            "metadata": {
                "type": "object",
                "description": "metadata object",
                "properties": many_string_properties(/*count*/ 300)
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        serde_json::to_value(schema).expect("serialize schema"),
        serde_json::json!({
            "type": "object",
            "properties": {
                "event": {
                    "type": "object",
                    "properties": {
                        "recurrence": {
                            "type": "object",
                            "properties": {
                                "pattern": {
                                    "type": "string"
                                }
                            }
                        }
                    }
                },
                "metadata": {}
            }
        })
    );
}

#[test]
fn parse_large_tool_input_schema_strips_descriptions_without_removing_description_property() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "description": "x".repeat(4_500),
        "properties": {
            "description": {
                "type": "string",
                "description": "User-facing description value"
            },
            "metadata": {
                "type": "object",
                "description": "Metadata object",
                "properties": {
                    "label": {
                        "type": "string",
                        "description": "Metadata label"
                    }
                }
            },
            "tags": {
                "type": "array",
                "description": "Tag list",
                "items": {
                    "type": "string",
                    "description": "Tag value"
                }
            },
            "extras": {
                "type": "object",
                "additionalProperties": {
                    "type": "string",
                    "description": "Extra value"
                }
            },
            "choice": {
                "description": "Choice value",
                "anyOf": [
                    {
                        "type": "string",
                        "description": "String choice"
                    },
                    {
                        "type": "number",
                        "description": "Number choice"
                    }
                ]
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        serde_json::to_value(schema).expect("serialize schema"),
        serde_json::json!({
            "type": "object",
            "properties": {
                "choice": {
                    "anyOf": [
                        {
                            "type": "string"
                        },
                        {
                            "type": "number"
                        }
                    ]
                },
                "description": {
                    "type": "string"
                },
                "extras": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": {
                        "type": "string"
                    }
                },
                "metadata": {
                    "type": "object",
                    "properties": {
                        "label": {
                            "type": "string"
                        }
                    }
                },
                "tags": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    }
                }
            }
        })
    );
}

#[test]
fn parse_large_tool_input_schema_preserves_object_enum_literal_descriptions() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "description": "x".repeat(4_500),
        "properties": {
            "choice": {
                "enum": [
                    {
                        "description": "first literal",
                        "id": 1
                    },
                    {
                        "description": "second literal",
                        "id": 2
                    }
                ]
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        serde_json::to_value(schema).expect("serialize schema"),
        serde_json::json!({
            "type": "object",
            "properties": {
                "choice": {
                    "type": "string",
                    "enum": [
                        {
                            "description": "first literal",
                            "id": 1
                        },
                        {
                            "description": "second literal",
                            "id": 2
                        }
                    ]
                }
            }
        })
    );
}

#[test]
fn collapse_deep_schema_objects_traverses_schema_children() {
    let mut schema = serde_json::json!({
        "type": "object",
        "properties": {
            "object_parent": {
                "type": "object",
                "properties": {
                    "complex": {
                        "type": "object",
                        "properties": {
                            "leaf": { "type": "string" }
                        }
                    },
                    "scalar": {
                        "type": "string"
                    }
                }
            },
            "array_parent": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "leaf": { "type": "string" }
                    }
                }
            },
            "map_parent": {
                "type": "object",
                "additionalProperties": {
                    "type": "object",
                    "properties": {
                        "leaf": { "type": "string" }
                    }
                }
            },
            "union_parent": {
                "anyOf": [
                    {
                        "type": "object",
                        "properties": {
                            "leaf": { "type": "string" }
                        }
                    },
                    { "type": "string" }
                ]
            }
        }
    });

    super::collapse_deep_schema_objects(&mut schema, /*depth*/ 0);

    assert_eq!(
        schema,
        serde_json::json!({
            "type": "object",
            "properties": {
                "object_parent": {
                    "type": "object",
                    "properties": {
                        "complex": {},
                        "scalar": {
                            "type": "string"
                        }
                    }
                },
                "array_parent": {
                    "type": "array",
                    "items": {}
                },
                "map_parent": {
                    "type": "object",
                    "additionalProperties": {}
                },
                "union_parent": {
                    "anyOf": [
                        {},
                        { "type": "string" }
                    ]
                }
            }
        })
    );
}

#[test]
fn parse_tool_input_schema_preserves_string_enum_constraints() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": {
    //     "response_length": { "type": "enum", "enum": ["short", "medium", "long"] },
    //     "kind": { "type": "const", "const": "tagged" },
    //     "scope": { "type": "enum", "enum": ["one", "two"] }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - Legacy `type: "enum"` and `type: "const"` inputs are normalized into
    //   the current string-enum representation.
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

#[test]
fn parse_tool_input_schema_preserves_refs_and_prunes_unreachable_defs() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": { "user": { "$ref": "#/$defs/User" } },
    //   "$defs": {
    //     "User": { "type": "object", "properties": { "name": { "type": "string" } } },
    //     "Unused": { "type": "string" }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - Local `$ref` is preserved as a schema hint.
    // - Reachable `$defs` entries stay attached to the root schema.
    // - Unreachable `$defs` entries are pruned.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "user": {"$ref": "#/$defs/User"}
        },
        "$defs": {
            "User": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                }
            },
            "Unused": {"type": "string"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)),
            properties: Some(BTreeMap::from([(
                "user".to_string(),
                JsonSchema {
                    schema_ref: Some("#/$defs/User".to_string()),
                    ..Default::default()
                },
            )])),
            defs: Some(BTreeMap::from([(
                "User".to_string(),
                JsonSchema::object(
                    BTreeMap::from([(
                        "name".to_string(),
                        JsonSchema::string(/*description*/ None),
                    )]),
                    /*required*/ None,
                    /*additional_properties*/ None,
                ),
            )])),
            ..Default::default()
        }
    );
}

#[test]
fn parse_tool_input_schema_preserves_refs_from_properties_named_def_tables() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": {
    //     "$defs": { "$ref": "#/$defs/User" }
    //   },
    //   "$defs": { "User": { "type": "string" }, "Unused": { "type": "boolean" } }
    // }
    //
    // Expected normalization behavior:
    // - A property named like the `$defs` keyword is treated as a user field
    //   while traversing `properties`.
    // - Refs from that property schema still mark root definitions reachable.
    // - Unreferenced root definitions are still pruned.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "$defs": {"$ref": "#/$defs/User"}
        },
        "$defs": {
            "User": {"type": "string"},
            "Unused": {"type": "boolean"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)),
            properties: Some(BTreeMap::from([(
                "$defs".to_string(),
                JsonSchema {
                    schema_ref: Some("#/$defs/User".to_string()),
                    ..Default::default()
                },
            )])),
            defs: Some(BTreeMap::from([(
                "User".to_string(),
                JsonSchema::string(/*description*/ None),
            )])),
            ..Default::default()
        }
    );
}

#[test]
fn parse_tool_input_schema_collects_refs_from_schema_child_keywords() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "items_holder": {
                "type": "array",
                "items": {"$ref": "#/$defs/Item"}
            },
            "map_holder": {
                "type": "object",
                "additionalProperties": {"$ref": "#/$defs/Extra"}
            },
            "choice": {
                "anyOf": [
                    {"$ref": "#/$defs/Choice"},
                    {"type": "string"}
                ]
            }
        },
        "$defs": {
            "Choice": {"type": "boolean"},
            "Extra": {"type": "number"},
            "Item": {"type": "string"},
            "Unused": {"type": "null"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        serde_json::to_value(schema).expect("serialize schema"),
        serde_json::json!({
            "type": "object",
            "properties": {
                "choice": {
                    "anyOf": [
                        {"$ref": "#/$defs/Choice"},
                        {"type": "string"}
                    ]
                },
                "items_holder": {
                    "type": "array",
                    "items": {"$ref": "#/$defs/Item"}
                },
                "map_holder": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": {"$ref": "#/$defs/Extra"}
                }
            },
            "$defs": {
                "Choice": {"type": "boolean"},
                "Extra": {"type": "number"},
                "Item": {"type": "string"}
            }
        })
    );
}

#[test]
fn parse_tool_input_schema_handles_cyclic_local_refs() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": { "node": { "$ref": "#/$defs/Node" } },
    //   "$defs": {
    //     "Node": {
    //       "type": "object",
    //       "properties": { "next": { "$ref": "#/$defs/Node" } }
    //     }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - Recursive refs are preserved.
    // - Pruning traversal terminates after visiting each local target once.
    // - Responses API handles this recursive local-ref shape correctly.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "node": {"$ref": "#/$defs/Node"}
        },
        "$defs": {
            "Node": {
                "type": "object",
                "properties": {
                    "next": {"$ref": "#/$defs/Node"}
                }
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)),
            properties: Some(BTreeMap::from([(
                "node".to_string(),
                JsonSchema {
                    schema_ref: Some("#/$defs/Node".to_string()),
                    ..Default::default()
                },
            )])),
            defs: Some(BTreeMap::from([(
                "Node".to_string(),
                JsonSchema::object(
                    BTreeMap::from([(
                        "next".to_string(),
                        JsonSchema {
                            schema_ref: Some("#/$defs/Node".to_string()),
                            ..Default::default()
                        },
                    )]),
                    /*required*/ None,
                    /*additional_properties*/ None,
                ),
            )])),
            ..Default::default()
        }
    );
}

#[test]
fn parse_tool_input_schema_preserves_legacy_definitions() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": { "user": { "$ref": "#/definitions/User" } },
    //   "definitions": {
    //     "User": { "type": "object", "properties": { "profile": { "$ref": "#/definitions/Profile" } } },
    //     "Profile": { "type": "object", "properties": { "name": { "type": "string" } } }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - Codex preserves legacy `definitions`.
    // - Reachability follows refs through the legacy definition table.
    // - Unreachable legacy definition entries are pruned.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "user": {"$ref": "#/definitions/User"}
        },
        "definitions": {
            "User": {
                "type": "object",
                "properties": {
                    "profile": {"$ref": "#/definitions/Profile"}
                }
            },
            "Profile": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                }
            },
            "Unused": {"type": "string"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)),
            properties: Some(BTreeMap::from([(
                "user".to_string(),
                JsonSchema {
                    schema_ref: Some("#/definitions/User".to_string()),
                    ..Default::default()
                },
            )])),
            definitions: Some(BTreeMap::from([
                (
                    "Profile".to_string(),
                    JsonSchema::object(
                        BTreeMap::from([(
                            "name".to_string(),
                            JsonSchema::string(/*description*/ None),
                        )]),
                        /*required*/ None,
                        /*additional_properties*/ None,
                    ),
                ),
                (
                    "User".to_string(),
                    JsonSchema::object(
                        BTreeMap::from([(
                            "profile".to_string(),
                            JsonSchema {
                                schema_ref: Some("#/definitions/Profile".to_string()),
                                ..Default::default()
                            },
                        )]),
                        /*required*/ None,
                        /*additional_properties*/ None,
                    ),
                ),
            ])),
            ..Default::default()
        }
    );
}

#[test]
fn parse_tool_input_schema_preserves_unresolved_and_external_refs() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": {
    //     "missing": { "$ref": "#/$defs/Missing" },
    //     "remote": { "$ref": "https://example.com/schema.json" }
    //   },
    //   "$defs": { "Unused": { "type": "string" } }
    // }
    //
    // Expected normalization behavior:
    // - Unresolved local refs and external refs are preserved.
    // - Unreachable local definitions are still pruned.
    // - Responses API handles these refs correctly during downstream validation.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "missing": {"$ref": "#/$defs/Missing"},
            "remote": {"$ref": "https://example.com/schema.json"}
        },
        "$defs": {
            "Unused": {"type": "string"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)),
            properties: Some(BTreeMap::from([
                (
                    "missing".to_string(),
                    JsonSchema {
                        schema_ref: Some("#/$defs/Missing".to_string()),
                        ..Default::default()
                    },
                ),
                (
                    "remote".to_string(),
                    JsonSchema {
                        schema_ref: Some("https://example.com/schema.json".to_string()),
                        ..Default::default()
                    },
                ),
            ])),
            ..Default::default()
        }
    );
}

#[test]
fn parse_tool_input_schema_preserves_nested_defs_ref_parent() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": { "name": { "$ref": "#/$defs/User/properties/name" } },
    //   "$defs": {
    //     "User": { "type": "object", "properties": { "name": { "type": "string" } } },
    //     "name": { "type": "string" },
    //     "Unused": { "type": "boolean" }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - The nested JSON Pointer ref remains unchanged.
    // - The parent root definition is retained so the local ref does not dangle.
    // - Unreferenced root definitions are still pruned.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "name": {"$ref": "#/$defs/User/properties/name"}
        },
        "$defs": {
            "User": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                }
            },
            "name": {"type": "string"},
            "Unused": {"type": "boolean"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)),
            properties: Some(BTreeMap::from([(
                "name".to_string(),
                JsonSchema {
                    schema_ref: Some("#/$defs/User/properties/name".to_string()),
                    ..Default::default()
                },
            )])),
            defs: Some(BTreeMap::from([(
                "User".to_string(),
                JsonSchema::object(
                    BTreeMap::from([(
                        "name".to_string(),
                        JsonSchema::string(/*description*/ None),
                    )]),
                    /*required*/ None,
                    /*additional_properties*/ None,
                ),
            )])),
            ..Default::default()
        }
    );
}

#[test]
fn parse_tool_input_schema_preserves_percent_encoded_definition_refs() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": {
    //     "user": { "$ref": "#/$defs/User%20Name" },
    //     "profile": { "$ref": "#/%24defs/Profile%7E0Name" }
    //   },
    //   "$defs": {
    //     "User Name": { "type": "string" },
    //     "Profile~Name": { "type": "string" },
    //     "Unused": { "type": "boolean" }
    //   }
    // }
    //
    // Expected normalization behavior:
    // - URI fragment percent encoding is decoded before JSON Pointer `~`
    //   escaping, per RFC 6901 section 6.
    // - The original `$ref` strings are preserved, but their definition
    //   targets are recognized as reachable and retained.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "user": {"$ref": "#/$defs/User%20Name"},
            "profile": {"$ref": "#/%24defs/Profile%7E0Name"}
        },
        "$defs": {
            "User Name": {"type": "string"},
            "Profile~Name": {"type": "string"},
            "Unused": {"type": "boolean"}
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)),
            properties: Some(BTreeMap::from([
                (
                    "profile".to_string(),
                    JsonSchema {
                        schema_ref: Some("#/%24defs/Profile%7E0Name".to_string()),
                        ..Default::default()
                    },
                ),
                (
                    "user".to_string(),
                    JsonSchema {
                        schema_ref: Some("#/$defs/User%20Name".to_string()),
                        ..Default::default()
                    },
                ),
            ])),
            defs: Some(BTreeMap::from([
                (
                    "Profile~Name".to_string(),
                    JsonSchema::string(/*description*/ None),
                ),
                (
                    "User Name".to_string(),
                    JsonSchema::string(/*description*/ None),
                ),
            ])),
            ..Default::default()
        }
    );
}

#[test]
fn parse_tool_input_schema_drops_malformed_definition_tables() {
    // Example schema shape:
    // {
    //   "type": "object",
    //   "properties": { "user": { "$ref": "#/$defs/User" } },
    //   "$defs": ["not", "an", "object"]
    // }
    //
    // Expected normalization behavior:
    // - Malformed `$defs` tables are dropped instead of rejecting the schema.
    // - The unresolved local ref remains visible to the model.
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "user": {"$ref": "#/$defs/User"}
        },
        "$defs": ["not", "an", "object"]
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema {
            schema_type: Some(JsonSchemaType::Single(JsonSchemaPrimitiveType::Object)),
            properties: Some(BTreeMap::from([(
                "user".to_string(),
                JsonSchema {
                    schema_ref: Some("#/$defs/User".to_string()),
                    ..Default::default()
                },
            )])),
            ..Default::default()
        }
    );
}
