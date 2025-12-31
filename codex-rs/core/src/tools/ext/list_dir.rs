//! Enhanced List Directory Tool Specification
//!
//! Extension module for list_dir tool with ignore file support.
//! Respects .gitignore and .ignore patterns.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create enhanced list_dir tool specification
///
/// Enhanced list_dir with:
/// - .gitignore and .ignore respect (always enabled)
/// - Depth control for recursive listing
/// - Pagination with offset/limit
/// - Directories-first sorting
/// - Symlink identification
pub fn create_enhanced_list_dir_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "dir_path".to_string(),
        JsonSchema::String {
            description: Some("Absolute path to the directory to list.".to_string()),
        },
    );

    properties.insert(
        "offset".to_string(),
        JsonSchema::Number {
            description: Some(
                "The entry number to start listing from. Must be 1 or greater.".to_string(),
            ),
        },
    );

    properties.insert(
        "limit".to_string(),
        JsonSchema::Number {
            description: Some("The maximum number of entries to return.".to_string()),
        },
    );

    properties.insert(
        "depth".to_string(),
        JsonSchema::Number {
            description: Some(
                "The maximum directory depth to traverse. Must be 1 or greater.".to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "list_dir".to_string(),
        description: "Lists entries in a local directory with ignore file support. \
            Returns entries with 1-indexed numbers, directories first, with symlink indicators."
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["dir_path".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_enhanced_list_dir_tool_spec() {
        let spec = create_enhanced_list_dir_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "list_dir");
        assert!(!tool.strict);
        assert!(tool.description.contains("ignore"));

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
        assert!(required.contains(&"dir_path".to_string()));

        // Check all properties exist
        assert!(properties.contains_key("dir_path"));
        assert!(properties.contains_key("offset"));
        assert!(properties.contains_key("limit"));
        assert!(properties.contains_key("depth"));
    }
}
