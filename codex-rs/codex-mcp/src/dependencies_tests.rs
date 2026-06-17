use std::collections::HashMap;

use pretty_assertions::assert_eq;

use super::McpServerDependencies;
use super::McpServerDependency;
use super::canonical_server_key;

#[test]
fn missing_from_deduplicates_by_transport_identity() {
    let dependency = McpServerDependency {
        source_name: "demo".to_string(),
        name: "docs".to_string(),
        transport: Some("streamable_http".to_string()),
        command: None,
        url: Some("https://example.com/mcp".to_string()),
    };
    let mut dependencies = McpServerDependencies::default();
    dependencies.push(dependency.clone());
    dependencies.push(dependency);

    let missing = dependencies.missing_from(&HashMap::new());

    assert_eq!(missing.len(), 1);
    assert_eq!(
        missing
            .get("docs")
            .map(|config| canonical_server_key("docs", config)),
        Some("mcp__streamable_http__https://example.com/mcp".to_string())
    );
}
