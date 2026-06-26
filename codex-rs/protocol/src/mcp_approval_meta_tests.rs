use pretty_assertions::assert_eq;

use super::McpToolSource;

#[test]
fn approval_source_preserves_guardian_wire_fields() {
    let source = McpToolSource::new(
        "source-1",
        "Documents",
        Some("Search company documents.".to_string()),
    )
    .expect("valid approval source");

    assert_eq!(
        serde_json::to_value(source).expect("serialize approval source"),
        serde_json::json!({
            "connector_id": "source-1",
            "connector_name": "Documents",
            "connector_description": "Search company documents.",
        })
    );
}
