use crate::openai_models::ModelMessages;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn memento_catalog_round_trip_preserves_sparse_tools_and_explicit_empty() {
    for tools in [
        json!({}),
        json!({
            "history": {"namespace_description": "", "read_item": {"description": "", "parameters": "not JSON"}},
            "notes": {"read_file": {"parameters": "{}"}},
            "context_management": {"new_context": {"description": "catalog"}}
        }),
    ] {
        let messages: ModelMessages = serde_json::from_value(json!({"tools": tools})).unwrap();
        assert_eq!(serde_json::to_value(messages.tools).unwrap(), tools);
    }
}
