use rmcp::model::JsonObject;
use serde_json::json;

pub(crate) fn list_input_schema() -> JsonObject {
    json_schema(json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "cursor": { "type": "string" },
            "max_results": { "type": "integer", "minimum": 1 }
        },
        "additionalProperties": false
    }))
}

pub(crate) fn list_output_schema() -> JsonObject {
    json_schema(json!({
        "type": "object",
        "properties": {
            "path": {
                "anyOf": [{ "type": "string" }, { "type": "null" }]
            },
            "next_cursor": {
                "anyOf": [{ "type": "string" }, { "type": "null" }]
            },
            "entries": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "entry_type": { "type": "string", "enum": ["file", "directory"] }
                    },
                    "required": ["path", "entry_type"],
                    "additionalProperties": false
                }
            },
            "truncated": { "type": "boolean" }
        },
        "required": ["path", "entries", "next_cursor", "truncated"],
        "additionalProperties": false
    }))
}

pub(crate) fn read_input_schema() -> JsonObject {
    json_schema(json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "line_offset": { "type": "integer", "minimum": 1 },
            "max_lines": { "type": "integer", "minimum": 1 }
        },
        "required": ["path"],
        "additionalProperties": false
    }))
}

pub(crate) fn read_output_schema() -> JsonObject {
    json_schema(json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "start_line_number": { "type": "integer" },
            "content": { "type": "string" },
            "truncated": { "type": "boolean" }
        },
        "required": ["path", "start_line_number", "content", "truncated"],
        "additionalProperties": false
    }))
}

pub(crate) fn search_input_schema() -> JsonObject {
    json_schema(json!({
        "type": "object",
        "properties": {
            "query": { "type": "string" },
            "path": { "type": "string" },
            "cursor": { "type": "string" },
            "context_lines": { "type": "integer", "minimum": 0 },
            "case_sensitive": { "type": "boolean" },
            "max_results": { "type": "integer", "minimum": 1 }
        },
        "required": ["query"],
        "additionalProperties": false
    }))
}

pub(crate) fn search_output_schema() -> JsonObject {
    json_schema(json!({
        "type": "object",
        "properties": {
            "query": { "type": "string" },
            "path": {
                "anyOf": [{ "type": "string" }, { "type": "null" }]
            },
            "next_cursor": {
                "anyOf": [{ "type": "string" }, { "type": "null" }]
            },
            "matches": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "match_line_number": { "type": "integer" },
                        "content_start_line_number": { "type": "integer" },
                        "content": { "type": "string" }
                    },
                    "required": ["path", "match_line_number", "content_start_line_number", "content"],
                    "additionalProperties": false
                }
            },
            "truncated": { "type": "boolean" }
        },
        "required": ["query", "path", "matches", "next_cursor", "truncated"],
        "additionalProperties": false
    }))
}

fn json_schema(value: serde_json::Value) -> JsonObject {
    serde_json::from_value(value)
        .unwrap_or_else(|err| panic!("static tool schema should deserialize: {err}"))
}
