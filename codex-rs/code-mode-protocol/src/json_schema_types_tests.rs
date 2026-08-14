use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn renders_recursive_local_refs_with_escaped_pointer_segments() {
    let schema = json!({
        "type": "object",
        "properties": {
            "clauses": {
                "type": "array",
                "items": { "$ref": "#/$defs/Boolean~1Clause~0v1" }
            }
        },
        "$defs": {
            "Boolean/Clause~v1": {
                "type": "object",
                "properties": {
                    "query": { "$ref": "#/$defs/Query" }
                }
            },
            "Query": {
                "oneOf": [
                    { "type": "string" },
                    {
                        "type": "object",
                        "properties": {
                            "clauses": {
                                "type": "array",
                                "items": { "$ref": "#/$defs/Boolean~1Clause~0v1" }
                            }
                        }
                    }
                ]
            }
        }
    });

    let rendered = render_json_schema_to_typescript(&schema);
    assert!(rendered.contains("clauses?: Array<{ query?: string | { clauses?: Array<{"));
    assert!(rendered.contains("query?: string | { clauses?: Array<unknown>; };"));
}

#[test]
fn renders_ref_siblings_uri_fragments_and_all_of_precedence() {
    assert_eq!(
        render_json_schema_to_typescript(&json!({
            "$ref": "#/$defs/Label",
            "enum": ["A"],
            "$defs": { "Label": { "type": "string" } }
        })),
        r#"(string) & ("A")"#
    );
    assert_eq!(
        render_json_schema_to_typescript(&json!({
            "$ref": "#/$defs/Foo%20Bar",
            "$defs": { "Foo Bar": { "type": "string" } }
        })),
        "string"
    );
    assert_eq!(
        render_json_schema_to_typescript(&json!({
            "allOf": [
                { "$ref": "#/$defs/Choice" },
                { "type": "object", "properties": { "value": { "type": "string" } } }
            ],
            "$defs": {
                "Choice": { "oneOf": [{ "type": "string" }, { "type": "number" }] }
            }
        })),
        "(string | number) & { value?: string; }"
    );
}

#[test]
fn leaves_local_refs_under_nested_schema_resources_unresolved() {
    let schema = json!({
        "$defs": {
            "Choice": { "type": "string" }
        },
        "type": "object",
        "properties": {
            "nested": {
                "$id": "urn:nested",
                "$defs": {
                    "Choice": { "type": "number" }
                },
                "type": "object",
                "properties": {
                    "value": { "$ref": "#/$defs/Choice" }
                }
            }
        }
    });

    assert_eq!(
        render_json_schema_to_typescript(&schema),
        "{ nested?: { value?: unknown; }; }"
    );
}

#[test]
fn bounds_expansions_without_charging_dangling_refs() {
    let mut properties = (0..MAX_TOTAL_LOCAL_REF_EXPANSIONS)
        .map(|index| {
            (
                format!("a_missing_{index}"),
                json!({ "$ref": format!("#/$defs/Missing{index}") }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    properties.insert("z_valid".to_string(), json!({ "$ref": "#/$defs/Valid" }));
    let schema = json!({
        "type": "object",
        "properties": properties,
        "$defs": { "Valid": { "type": "string" } }
    });

    let rendered = render_json_schema_to_typescript(&schema);
    assert!(rendered.contains("z_valid?: string;"));

    let properties = (0..MAX_TOTAL_LOCAL_REF_EXPANSIONS + 2)
        .map(|index| {
            (
                format!("property_{index}"),
                json!({ "$ref": "#/$defs/Item" }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let rendered = render_json_schema_to_typescript(&json!({
        "type": "object",
        "properties": properties,
        "$defs": { "Item": { "type": "string" } }
    }));
    assert_eq!(
        rendered.matches("string").count(),
        MAX_TOTAL_LOCAL_REF_EXPANSIONS
    );
    assert_eq!(rendered.matches("unknown").count(), 2);
}

#[test]
fn repeated_large_ref_expansions_exhaust_render_work_budget() {
    let properties = (0..MAX_TOTAL_LOCAL_REF_EXPANSIONS)
        .map(|index| {
            (
                format!("property_{index}"),
                json!({ "$ref": "#/$defs/Item" }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let schema = json!({
        "type": "object",
        "properties": properties,
        "$defs": {
            "Item": {
                "type": "object",
                "properties": {
                    "value": {
                        "type": "string",
                        "description": "x".repeat(MAX_RENDERED_SCHEMA_BYTES / 2)
                    }
                }
            }
        }
    });

    let mut renderer = JsonSchemaTypeRenderer::new(&schema);
    assert_eq!(renderer.render(&schema), "unknown");
    assert!(renderer.render_work_budget_exhausted);
}

#[test]
fn oversized_ref_literal_exhausts_render_work_budget() {
    let schema = json!({
        "$ref": "#/$defs/Value",
        "$defs": {
            "Value": {
                "const": "x".repeat(MAX_RENDER_WORK_BYTES)
            }
        }
    });

    let mut renderer = JsonSchemaTypeRenderer::new(&schema);
    assert_eq!(renderer.render(&schema), "unknown");
    assert!(renderer.render_work_budget_exhausted);
}

#[test]
fn rendered_schema_has_a_hard_size_cap() {
    let description = "x".repeat(MAX_RENDERED_SCHEMA_BYTES);
    let schema = json!({
        "type": "object",
        "properties": {
            "value": { "type": "string", "description": description }
        }
    });

    assert_eq!(render_json_schema_to_typescript(&schema), "unknown");
}
