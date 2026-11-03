#![cfg(test)]

use super::invocation_context;
use rmcp::model::Tool;
use serde_json::json;

#[test]
fn discovery_preserves_the_connectors_opaque_invocation_context() {
    let mut declaration = json!({
        "name": "search_service.web_run",
        "inputSchema": {"type": "object"},
    });
    let tool: Tool = serde_json::from_value(declaration.clone()).unwrap();
    assert_eq!(invocation_context(&tool), None);

    let pin = json!({"revision": "connector-selected"});
    declaration["_meta"] = json!({"_codex_apps": {"invocation_context": pin}});
    let tool: Tool = serde_json::from_value(declaration).unwrap();
    assert_eq!(invocation_context(&tool), Some(&pin));
}
