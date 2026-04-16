use super::AdditionalProperties;
use super::JsonSchema;
use super::JsonSchemaPrimitiveType;
use super::JsonSchemaType;
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

    assert_eq!(schema, JsonSchema::string(/*description*/ None));
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
fn parse_tool_input_schema_defaults_empty_schema_to_string() {
    // Example schema shape:
    // {}
    //
    // Expected normalization behavior:
    // - With no structural hints at all, the normalizer falls back to a
    //   permissive string schema.
    let schema = parse_tool_input_schema(&serde_json::json!({})).expect("parse schema");

    assert_eq!(schema, JsonSchema::string(/*description*/ None));
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

#[test]
fn parse_tool_input_schema_resolves_local_ref_objects() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "start": { "$ref": "#/$defs/date_time_zone" }
        },
        "$defs": {
            "date_time_zone": {
                "type": "object",
                "properties": {
                    "dateTime": { "type": "string" },
                    "timeZone": { "type": "string" }
                },
                "required": ["dateTime", "timeZone"]
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "start".to_string(),
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
                    /*additional_properties*/ None,
                ),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_merges_local_ref_sibling_constraints() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "start": {
                "$ref": "#/$defs/date_time_zone",
                "properties": {
                    "timeZone": {
                        "enum": ["UTC"]
                    },
                    "calendar": {
                        "type": "string"
                    }
                },
                "required": ["timeZone", "calendar"]
            }
        },
        "$defs": {
            "date_time_zone": {
                "type": "object",
                "properties": {
                    "dateTime": { "type": "string" },
                    "timeZone": { "type": "string" }
                },
                "required": ["dateTime"]
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "start".to_string(),
                JsonSchema::object(
                    BTreeMap::from([
                        (
                            "calendar".to_string(),
                            JsonSchema::string(/*description*/ None),
                        ),
                        (
                            "dateTime".to_string(),
                            JsonSchema::string(/*description*/ None),
                        ),
                        (
                            "timeZone".to_string(),
                            JsonSchema::string_enum(
                                vec![serde_json::json!("UTC")],
                                /*description*/ None,
                            ),
                        ),
                    ]),
                    Some(vec![
                        "dateTime".to_string(),
                        "timeZone".to_string(),
                        "calendar".to_string(),
                    ]),
                    /*additional_properties*/ None,
                ),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_handles_cyclic_local_refs_without_recursing_forever() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "node": { "$ref": "#/$defs/node" }
        },
        "$defs": {
            "node": {
                "type": "object",
                "properties": {
                    "next": { "$ref": "#/$defs/node" }
                },
                "required": ["next"]
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "node".to_string(),
                JsonSchema::object(
                    BTreeMap::from([(
                        "next".to_string(),
                        JsonSchema::object(
                            BTreeMap::new(),
                            /*required*/ None,
                            /*additional_properties*/ None,
                        ),
                    )]),
                    Some(vec!["next".to_string()]),
                    /*additional_properties*/ None,
                ),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_merges_outer_constraints_when_unwrapping_single_variant_combiners() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "wrapped": {
                "allOf": [{
                    "type": "object",
                    "properties": {
                        "a": { "type": "string" }
                    },
                    "required": ["a"]
                }],
                "properties": {
                    "b": { "type": "string" }
                },
                "required": ["b"]
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "wrapped".to_string(),
                JsonSchema::object(
                    BTreeMap::from([
                        ("a".to_string(), JsonSchema::string(/*description*/ None)),
                        ("b".to_string(), JsonSchema::string(/*description*/ None)),
                    ]),
                    Some(vec!["a".to_string(), "b".to_string()]),
                    /*additional_properties*/ None,
                ),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_merges_outer_constraints_when_unwrapping_true_combiners() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "wrapped": {
                "allOf": [true],
                "type": "object",
                "properties": {
                    "b": { "type": "string" }
                },
                "required": ["b"]
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "wrapped".to_string(),
                JsonSchema::object(
                    BTreeMap::from([("b".to_string(), JsonSchema::string(/*description*/ None),)]),
                    Some(vec!["b".to_string()]),
                    /*additional_properties*/ None,
                ),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_preserves_sibling_constraints_for_true_ref_targets() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "config": {
                "$ref": "#/$defs/anything",
                "type": "object",
                "properties": {
                    "dateTime": { "type": "string" }
                },
                "required": ["dateTime"]
            }
        },
        "$defs": {
            "anything": true
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "config".to_string(),
                JsonSchema::object(
                    BTreeMap::from([(
                        "dateTime".to_string(),
                        JsonSchema::string(/*description*/ None),
                    )]),
                    Some(vec!["dateTime".to_string()]),
                    /*additional_properties*/ None,
                ),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_intersects_enums_when_merging_ref_siblings() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "timeZone": {
                "$ref": "#/$defs/supported_time_zone",
                "enum": ["UTC", "PST"]
            }
        },
        "$defs": {
            "supported_time_zone": {
                "enum": ["UTC", "EST"]
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "timeZone".to_string(),
                JsonSchema::string_enum(vec![serde_json::json!("UTC")], /*description*/ None),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_narrows_number_and_integer_type_intersections() {
    let schema = parse_tool_input_schema(&serde_json::json!({
        "type": "object",
        "properties": {
            "count": {
                "$ref": "#/$defs/number_like",
                "type": "integer"
            }
        },
        "$defs": {
            "number_like": {
                "type": "number"
            }
        }
    }))
    .expect("parse schema");

    assert_eq!(
        schema,
        JsonSchema::object(
            BTreeMap::from([(
                "count".to_string(),
                JsonSchema::integer(/*description*/ None),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
    );
}

#[test]
fn parse_tool_input_schema_unwraps_single_variant_all_of_objects() {
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
                    "required": ["dateTime", "timeZone"]
                }]
            }
        }
    }))
    .expect("parse schema");

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
                    /*additional_properties*/ None,
                ),
            )]),
            /*required*/ None,
            /*additional_properties*/ None
        )
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
            )]),
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
