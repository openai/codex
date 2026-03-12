use super::render_json_schema_to_typescript;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn render_json_schema_to_typescript_renders_object_properties() {
    let schema = json!({
        "type": "object",
        "properties": {
            "path": {"type": "string"},
            "recursive": {"type": "boolean"}
        },
        "required": ["path"],
        "additionalProperties": false
    });

    assert_eq!(
        render_json_schema_to_typescript(&schema),
        "{\n  path: string;\n  recursive?: boolean;\n}"
    );
}

#[test]
fn render_json_schema_to_typescript_renders_anyof_unions() {
    let schema = json!({
        "anyOf": [
            {"const": "pending"},
            {"const": "done"},
            {"type": "number"}
        ]
    });

    assert_eq!(
        render_json_schema_to_typescript(&schema),
        "\"pending\" | \"done\" | number"
    );
}

#[test]
fn render_json_schema_to_typescript_renders_additional_properties() {
    let schema = json!({
        "type": "object",
        "properties": {
            "tags": {
                "type": "array",
                "items": {"type": "string"}
            }
        },
        "additionalProperties": {"type": "integer"}
    });

    assert_eq!(
        render_json_schema_to_typescript(&schema),
        "{\n  tags?: Array<string>;\n  [key: string]: number;\n}"
    );
}

#[test]
fn render_json_schema_to_typescript_sorts_object_properties() {
    let schema = json!({
        "type": "object",
        "properties": {
            "structuredContent": {"type": "string"},
            "_meta": {"type": "string"},
            "isError": {"type": "boolean"},
            "content": {"type": "array", "items": {"type": "string"}}
        },
        "required": ["content"]
    });

    assert_eq!(
        render_json_schema_to_typescript(&schema),
        "{\n  _meta?: string;\n  content: Array<string>;\n  isError?: boolean;\n  structuredContent?: string;\n}"
    );
}
