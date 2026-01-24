//! Web Fetch Tool Specification
//!
//! Fetches content from URLs and converts HTML to plain text.
//! Based on gemini-cli web-fetch.ts design.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create Web Fetch tool specification
///
/// Web Fetch fetches content from URLs with:
/// - HTTP/HTTPS protocol support only
/// - HTML to plain text conversion
/// - GitHub blob URL to raw URL conversion
/// - Content truncation (100k chars max)
/// - 10 second timeout
pub fn create_web_fetch_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "url".to_string(),
        JsonSchema::String {
            description: Some(
                "The URL to fetch. Must start with http:// or https://. \
                 GitHub blob URLs are automatically converted to raw URLs."
                    .to_string(),
            ),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "web_fetch".to_string(),
        description: "Fetches content from a URL and returns it as plain text. \
             HTML is automatically converted to readable text format. \
             Use for reading web pages, documentation, or API responses. \
             Supports http:// and https:// URLs only."
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["url".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_web_fetch_tool_spec() {
        let spec = create_web_fetch_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "web_fetch");
        assert!(!tool.strict);
        assert!(tool.description.contains("Fetches content from a URL"));
        assert!(tool.description.contains("plain text"));

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
        assert!(required.contains(&"url".to_string()));

        // Check url property exists
        assert!(properties.contains_key("url"));
    }

    #[test]
    fn test_url_description() {
        let spec = create_web_fetch_tool();
        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        let JsonSchema::Object { properties, .. } = tool.parameters else {
            panic!("Expected object parameters");
        };

        let url_schema = &properties["url"];
        if let JsonSchema::String {
            description: Some(desc),
        } = url_schema
        {
            assert!(desc.contains("http://"));
            assert!(desc.contains("https://"));
            assert!(desc.contains("GitHub"));
        } else {
            panic!("url should have description");
        }
    }
}
