//! Rich Grep Tool Specification
//!
//! Extension module for Rich Grep tool using ripgrep JSON mode.
//! Returns matching lines with file paths and line numbers.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create Rich Grep tool specification
///
/// Rich Grep searches file contents using ripgrep with:
/// - Pattern matching (regex or literal)
/// - Context lines support (-A/-B/-C)
/// - Case sensitivity option
/// - .gitignore and .ignore respect (always enabled)
/// - Modification time sorting (recent files first)
/// - Rich output with file path + line number + line content
pub fn create_ripgrep_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    // Required - pattern
    properties.insert(
        "pattern".to_string(),
        JsonSchema::String {
            description: Some(
                "Regex pattern to search for. Use \\\\b for word boundaries. \
                 Example: \"fn\\\\s+main\" matches function declarations."
                    .to_string(),
            ),
        },
    );

    // Optional - directory/filter
    properties.insert(
        "path".to_string(),
        JsonSchema::String {
            description: Some(
                "Directory or file to search in. Defaults to current working directory."
                    .to_string(),
            ),
        },
    );

    properties.insert(
        "include".to_string(),
        JsonSchema::String {
            description: Some(
                "Glob pattern to filter files (e.g., \"*.rs\", \"src/**/*.ts\").".to_string(),
            ),
        },
    );

    // Optional - search options
    properties.insert(
        "case_sensitive".to_string(),
        JsonSchema::Boolean {
            description: Some(
                "If true, search is case-sensitive. Default: false (case-insensitive).".to_string(),
            ),
        },
    );

    properties.insert(
        "fixed_strings".to_string(),
        JsonSchema::Boolean {
            description: Some(
                "If true, treat pattern as literal string instead of regex. Default: false."
                    .to_string(),
            ),
        },
    );

    // Optional - context lines
    properties.insert(
        "context".to_string(),
        JsonSchema::Number {
            description: Some("Lines of context before and after each match (-C).".to_string()),
        },
    );

    properties.insert(
        "after".to_string(),
        JsonSchema::Number {
            description: Some("Lines of context after each match (-A).".to_string()),
        },
    );

    properties.insert(
        "before".to_string(),
        JsonSchema::Number {
            description: Some("Lines of context before each match (-B).".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "grep_files".to_string(),
        description: "Search file contents using ripgrep. Returns matching lines with file paths \
             and line numbers, sorted by modification time (newest first)."
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
    fn test_create_ripgrep_tool_spec() {
        let spec = create_ripgrep_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "grep_files");
        assert!(!tool.strict);
        assert!(tool.description.contains("ripgrep"));

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
        assert!(properties.contains_key("include"));
        assert!(properties.contains_key("case_sensitive"));
        assert!(properties.contains_key("fixed_strings"));
        assert!(properties.contains_key("context"));
        assert!(properties.contains_key("after"));
        assert!(properties.contains_key("before"));
    }

    #[test]
    fn test_pattern_is_required() {
        let spec = create_ripgrep_tool();
        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        let JsonSchema::Object { required, .. } = tool.parameters else {
            panic!("Expected object parameters");
        };

        let required = required.expect("Should have required fields");
        assert!(required.contains(&"pattern".to_string()));
    }
}
