//! Think Tool Specification
//!
//! A no-op tool for logging thoughts. Inspired by tau-bench think tool.
//! Useful for complex reasoning, brainstorming, and planning without
//! making any changes or obtaining new information.

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create Think tool specification
///
/// Think is a no-op tool that logs thoughts for transparency.
/// Common use cases:
/// - Brainstorming bug fixes after discovering the source
/// - Planning complex refactoring approaches
/// - Designing new feature architecture
/// - Organizing debugging hypotheses
pub fn create_think_tool() -> ToolSpec {
    let mut properties = BTreeMap::new();

    properties.insert(
        "thought".to_string(),
        JsonSchema::String {
            description: Some("Your thoughts.".to_string()),
        },
    );

    ToolSpec::Function(ResponsesApiTool {
        name: "think".to_string(),
        description: r#"Use the tool to think about something. It will not obtain new information or make any changes to the repository, but just log the thought. Use it when complex reasoning or brainstorming is needed.

Common use cases:
1. When exploring a repository and discovering the source of a bug, call this tool to brainstorm several unique ways of fixing the bug, and assess which change(s) are likely to be simplest and most effective
2. After receiving test results, use this tool to brainstorm ways to fix failing tests
3. When planning a complex refactoring, use this tool to outline different approaches and their tradeoffs
4. When designing a new feature, use this tool to think through architecture decisions and implementation details
5. When debugging a complex issue, use this tool to organize your thoughts and hypotheses

The tool simply logs your thought process for better transparency and does not execute any code or make changes."#
            .to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["thought".to_string()]),
            additional_properties: Some(false.into()),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_think_tool_spec() {
        let spec = create_think_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, "think");
        assert!(!tool.strict);
        assert!(tool.description.contains("think about something"));
        assert!(tool.description.contains("Common use cases"));
        assert!(tool.description.contains("brainstorm"));

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
        assert!(required.contains(&"thought".to_string()));

        // Check thought property exists
        assert!(properties.contains_key("thought"));
    }

    #[test]
    fn test_thought_description() {
        let spec = create_think_tool();
        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        let JsonSchema::Object { properties, .. } = tool.parameters else {
            panic!("Expected object parameters");
        };

        let thought_schema = &properties["thought"];
        if let JsonSchema::String {
            description: Some(desc),
        } = thought_schema
        {
            assert!(desc.contains("thought"));
        } else {
            panic!("thought should have description");
        }
    }
}
