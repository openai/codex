use super::append_code_mode_sample;
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

#[test]
fn append_code_mode_sample_uses_static_import_for_valid_identifiers() {
    assert_eq!(
        append_code_mode_sample(
            "desc",
            "mcp__ologs__get_profile",
            "args",
            "{ foo: string }".to_string(),
            "unknown".to_string(),
        ),
        "desc\n\nCode mode declaration:\n```ts\nimport { get_profile } from \"tools/mcp/ologs.js\";\ndeclare function get_profile(args: { foo: string }): Promise<unknown>;\n```"
    );
}

#[test]
fn append_code_mode_sample_uses_dynamic_import_for_non_identifier_tool_names() {
    assert_eq!(
        append_code_mode_sample(
            "desc",
            "mcp__rmcp__echo-tool",
            "args",
            "{ foo: string }".to_string(),
            "unknown".to_string(),
        ),
        "desc\n\nCode mode declaration:\n```ts\nconst { [\"echo-tool\"]: echo_tool } = await import(\"tools/mcp/rmcp.js\");\ndeclare function echo_tool(args: { foo: string }): Promise<unknown>;\n```"
    );
}
