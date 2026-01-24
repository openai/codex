//! Glob Files Tool Specification
//!
//! Extension module for Glob Files tool.
//! Find files by pattern matching with agent ignore support.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create Glob Files tool specification
///
/// Glob Files finds files by name pattern with:
/// - Glob pattern matching (e.g., "**/*.rs")
/// - Case sensitivity option
/// - .gitignore and .ignore respect (always enabled)
/// - Modification time sorting (recent files first)
pub fn create_glob_files_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "pattern".to_string(),
        JsonSchema::String {
            description: Some(
                "Glob pattern to match files (e.g., \"**/*.rs\", \"src/**/*.ts\", \"*.txt\"). \
                 Supports ** for recursive matching."
                    .to_string(),
            ),
        },
    );

    properties.insert(
        "path".to_string(),
        JsonSchema::String {
            description: Some(
                "Directory to search in. Defaults to current working directory if not specified."
                    .to_string(),
            ),
        },
    );

    properties.insert(
        "case_sensitive".to_string(),
        JsonSchema::Boolean {
            description: Some(
                "Whether pattern matching is case-sensitive. Defaults to false.".to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "glob_files".to_string(),
        description:
            "Find files by glob pattern. Returns matching file paths sorted by modification time \
             (newest first)."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["pattern".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_glob_files_tool_spec() {
        let spec = create_glob_files_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "glob_files");
        assert!(!tool.strict);
        assert!(tool.description.contains("glob pattern"));

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
        assert!(required.contains(&"pattern".to_string()));

        // Check all properties exist
        assert!(properties.contains_key("pattern"));
        assert!(properties.contains_key("path"));
        assert!(properties.contains_key("case_sensitive"));
    }

    #[test]
    fn test_pattern_description() {
        let spec = create_glob_files_tool();
        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        let JsonSchema::Object { properties, .. } = tool.parameters else {
            panic!("Expected object parameters");
        };

        let pattern_schema = &properties["pattern"];
        if let JsonSchema::String {
            description: Some(desc),
        } = pattern_schema
        {
            assert!(desc.contains("**/*.rs"));
        } else {
            panic!("pattern should have description");
        }
    }
}
