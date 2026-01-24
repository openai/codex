//! RepoMap Tool Specification
//!
//! Extension module for generating repository structure maps.
//! Produces a condensed view of the codebase showing files and key symbols.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create RepoMap tool specification
///
/// RepoMap generates a condensed map of the codebase:
/// - File tree with key symbols (functions, classes, etc.)
/// - PageRank-weighted symbol selection
/// - Token-budgeted output
pub fn create_repomap_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    // Optional - max_tokens
    properties.insert(
        "max_tokens".to_string(),
        JsonSchema::Number {
            description: Some(
                "Maximum tokens for the output. Default: 1024, Max: 8192. \
                 Higher values include more files and symbols."
                    .to_string(),
            ),
        },
    );

    // Optional - symbols
    properties.insert(
        "symbols".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String {
                description: Some("Symbol name (function, class, type)".to_string()),
            }),
            description: Some(
                "Focus on specific symbols. Files containing these symbols \
                 get 10x weight boost for inclusion."
                    .to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "repomap".to_string(),
        description: "Generate a condensed map of the codebase structure. \
             Shows files with key symbols (functions, classes, types). \
             Use this to understand the overall architecture and \
             locate relevant code areas before diving into details."
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: None, // All parameters are optional
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_repomap_tool_spec() {
        let spec = create_repomap_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "repomap");
        assert!(!tool.strict);
        assert!(tool.description.contains("map"));

        let JsonSchema::Object {
            properties,
            required,
            ..
        } = tool.parameters
        else {
            panic!("Expected object parameters");
        };

        // No required fields
        assert!(required.is_none());

        // Check properties
        assert!(properties.contains_key("max_tokens"));
        assert!(properties.contains_key("symbols"));
    }
}
