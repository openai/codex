#![cfg(test)]

use crate::openai_models::ModelMessages;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn plugin_messages_preserve_sparse_nested_and_literal_wire_values() {
    for plugins in [
        serde_json::Value::Null,
        json!({}),
        json!({"generic": {}, "selected": {}, "recommendations": {}}),
        json!({
            "generic": {"intro": "", "usage_instructions": " \r\n\t"},
            "selected": {
                "intro": "  {{ plugin_name }}\r\n",
                "search_tools": "literal {{braces}}",
                "skill_namespace": "{{ skill_namespace }}",
                "apps": "{{ apps }}",
                "mcp_servers": "{{ mcp_servers }}",
                "closing": "é".repeat(251),
                "omitted": "omitted"
            },
            "recommendations": {"intro": "recommendations"}
        }),
    ] {
        let messages: ModelMessages = serde_json::from_value(json!({"plugins": plugins})).unwrap();
        let mut expected = serde_json::to_value(ModelMessages::default()).unwrap();
        if !plugins.is_null() {
            expected["plugins"] = plugins;
        }
        assert_eq!(serde_json::to_value(messages).unwrap(), expected);
    }
}
