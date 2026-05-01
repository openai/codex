use rmcp::model::JsonObject;
use serde_json::json;

pub(crate) fn list_input_schema() -> JsonObject {
    json_schema(json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
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
        "required": ["path", "entries", "truncated"],
        "additionalProperties": false
    }))
}

pub(crate) fn read_input_schema() -> JsonObject {
    json_schema(json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" }
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
            "content": { "type": "string" },
            "truncated": { "type": "boolean" }
        },
        "required": ["path", "content", "truncated"],
        "additionalProperties": false
    }))
}

pub(crate) fn search_input_schema() -> JsonObject {
    json_schema(json!({
        "type": "object",
        "properties": {
            "query": { "type": "string" },
            "path": { "type": "string" },
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
            "matches": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "line_number": { "type": "integer" },
                        "line": { "type": "string" }
                    },
                    "required": ["path", "line_number", "line"],
                    "additionalProperties": false
                }
            },
            "truncated": { "type": "boolean" }
        },
        "required": ["query", "path", "matches", "truncated"],
        "additionalProperties": false
    }))
}

fn json_schema(value: serde_json::Value) -> JsonObject {
    serde_json::from_value(value)
        .unwrap_or_else(|err| panic!("static tool schema should deserialize: {err}"))
}
