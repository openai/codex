//! Code Search Tool Specification
//!
//! Extension module for semantic code search using the retrieval system.
//! Searches indexed codebase using BM25 and optional vector similarity.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create Code Search tool specification
///
/// Code Search searches indexed codebase with:
/// - BM25 full-text search
/// - Optional vector similarity (if embeddings configured)
/// - Query rewriting for non-English queries
/// - Result ranking with RRF fusion
pub fn create_code_search_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    // Required - query
    properties.insert(
        "query".to_string(),
        JsonSchema::String {
            description: Some(
                "Search query. Can be natural language or code identifiers. \
                 Non-English queries are automatically translated."
                    .to_string(),
            ),
        },
    );

    // Optional - limit
    properties.insert(
        "limit".to_string(),
        JsonSchema::Number {
            description: Some(
                "Maximum number of results to return. Default: 10, Max: 50.".to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "code_search".to_string(),
        description: "Search the indexed codebase for relevant code snippets. \
             Returns matching code chunks with file paths and line numbers. \
             Use this when you need to find specific code patterns, functions, \
             or understand how features are implemented."
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["query".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_code_search_tool_spec() {
        let spec = create_code_search_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "code_search");
        assert!(!tool.strict);
        assert!(tool.description.contains("Search"));

        let JsonSchema::Object {
            properties,
            required,
            ..
        } = tool.parameters
        else {
            panic!("Expected object parameters");
        };

        // Check required fields
        let required = required.expect("Should have required fields");
        assert_eq!(required.len(), 1);
        assert!(required.contains(&"query".to_string()));

        // Check properties
        assert!(properties.contains_key("query"));
        assert!(properties.contains_key("limit"));
    }
}
