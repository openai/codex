//! Web Search Tool Specification
//!
//! Performs web searches using configurable backends (DuckDuckGo, Tavily).
//! Based on gemini-cli web-search.ts design with citation support.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create Web Search tool specification
///
/// Web Search performs searches with:
/// - Multiple provider support (DuckDuckGo free, Tavily paid)
/// - Formatted markdown results with citations
/// - Source attribution and URLs
/// - Configurable max results (1-20)
pub fn create_web_search_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "query".to_string(),
        JsonSchema::String {
            description: Some(
                "The search query to find information on the web. \
                 Be specific and include relevant keywords."
                    .to_string(),
            ),
        },
    );

    properties.insert(
        "max_results".to_string(),
        JsonSchema::Number {
            description: Some(
                "Maximum number of results to return (1-20). \
                 Defaults to configured value (usually 5)."
                    .to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "web_search".to_string(),
        description: "Searches the web for current information using DuckDuckGo or Tavily. \
             Returns summarized results with sources and citations. \
             Use for recent events, documentation, or information beyond training data."
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
    fn test_create_web_search_tool_spec() {
        let spec = create_web_search_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "web_search");
        assert!(!tool.strict);
        assert!(tool.description.contains("Searches the web"));
        assert!(tool.description.contains("DuckDuckGo"));
        assert!(tool.description.contains("Tavily"));

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

        // Check properties exist
        assert!(properties.contains_key("query"));
        assert!(properties.contains_key("max_results"));
    }

    #[test]
    fn test_query_description() {
        let spec = create_web_search_tool();
        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        let JsonSchema::Object { properties, .. } = tool.parameters else {
            panic!("Expected object parameters");
        };

        let query_schema = &properties["query"];
        if let JsonSchema::String {
            description: Some(desc),
        } = query_schema
        {
            assert!(desc.contains("search query"));
            assert!(desc.contains("keywords"));
        } else {
            panic!("query should have description");
        }
    }

    #[test]
    fn test_max_results_is_optional() {
        let spec = create_web_search_tool();
        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        let JsonSchema::Object { required, .. } = tool.parameters else {
            panic!("Expected object parameters");
        };

        let required = required.expect("Should have required fields");
        // max_results should NOT be required
        assert!(!required.contains(&"max_results".to_string()));
    }
}
