//! Write File Tool Specification
//!
//! Extension module for Write File tool.
//! Creates new files or overwrites existing files with specified content.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create Write File tool specification
///
/// Write File creates or overwrites files with:
/// - Automatic parent directory creation
/// - Path validation (within workspace)
/// - Detailed error messages
pub fn create_write_file_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "file_path".to_string(),
        JsonSchema::String {
            description: Some(
                "Absolute path to the file to write. Parent directories will be created if needed."
                    .to_string(),
            ),
        },
    );

    properties.insert(
        "content".to_string(),
        JsonSchema::String {
            description: Some("The content to write to the file.".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "write_file".to_string(),
        description:
            "Creates a new file or overwrites an existing file with the specified content. \
             Parent directories are created automatically if they don't exist. \
             Use absolute paths."
                .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["file_path".to_string(), "content".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_write_file_tool_spec() {
        let spec = create_write_file_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "write_file");
        assert!(!tool.strict);
        assert!(tool.description.contains("Creates a new file"));
        assert!(tool.description.contains("overwrites"));

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
        assert_eq!(required.len(), 2);
        assert!(required.contains(&"file_path".to_string()));
        assert!(required.contains(&"content".to_string()));

        // Check all properties exist
        assert!(properties.contains_key("file_path"));
        assert!(properties.contains_key("content"));
    }

    #[test]
    fn test_file_path_description() {
        let spec = create_write_file_tool();
        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        let JsonSchema::Object { properties, .. } = tool.parameters else {
            panic!("Expected object parameters");
        };

        let file_path_schema = &properties["file_path"];
        if let JsonSchema::String {
            description: Some(desc),
        } = file_path_schema
        {
            assert!(desc.contains("Absolute path"));
            assert!(desc.contains("Parent directories"));
        } else {
            panic!("file_path should have description");
        }
    }
}
