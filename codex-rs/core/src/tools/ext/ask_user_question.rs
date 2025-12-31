//! AskUserQuestion Tool Specification
//!
//! Tool for asking the user questions during execution.
//! Aligned with Claude Code's AskUserQuestion tool (chunks.153.mjs).

use crate::client_common::tools::ResponsesApiTool;
use crate::client_common::tools::ToolSpec;
use crate::tools::names;
use crate::tools::spec::JsonSchema;
use std::collections::BTreeMap;

/// Create ask_user_question tool specification.
///
/// This tool allows the LLM to ask the user questions during execution
/// for clarification, gathering preferences, or making decisions.
pub fn create_ask_user_question_tool() -> ToolSpec {
    // Build option schema
    let option_properties = BTreeMap::from([
        (
            "label".to_string(),
            JsonSchema::String {
                description: Some("Display text for this option (1-5 words)".to_string()),
            },
        ),
        (
            "description".to_string(),
            JsonSchema::String {
                description: Some("Explanation of what this option means".to_string()),
            },
        ),
    ]);

    // Build question schema
    let question_properties = BTreeMap::from([
        (
            "question".to_string(),
            JsonSchema::String {
                description: Some(
                    "The complete question to ask the user. Should end with ?".to_string(),
                ),
            },
        ),
        (
            "header".to_string(),
            JsonSchema::String {
                description: Some("Very short label displayed as a tag (max 12 chars)".to_string()),
            },
        ),
        (
            "options".to_string(),
            JsonSchema::Array {
                items: Box::new(JsonSchema::Object {
                    properties: option_properties,
                    required: Some(vec!["label".to_string(), "description".to_string()]),
                    additional_properties: Some(false.into()),
                }),
                description: Some("2-4 options for this question".to_string()),
            },
        ),
        (
            "multiSelect".to_string(),
            JsonSchema::Boolean {
                description: Some("Allow multiple answers to be selected".to_string()),
            },
        ),
    ]);

    // Build top-level parameters
    let parameters = JsonSchema::Object {
        properties: BTreeMap::from([(
            "questions".to_string(),
            JsonSchema::Array {
                items: Box::new(JsonSchema::Object {
                    properties: question_properties,
                    required: Some(vec![
                        "question".to_string(),
                        "header".to_string(),
                        "options".to_string(),
                        "multiSelect".to_string(),
                    ]),
                    additional_properties: Some(false.into()),
                }),
                description: Some("1-4 questions to ask the user".to_string()),
            },
        )]),
        required: Some(vec!["questions".to_string()]),
        additional_properties: Some(false.into()),
    };

    ToolSpec::Function(ResponsesApiTool {
        name: names::ASK_USER_QUESTION.to_string(),
        description: r#"Use this tool when you need to ask the user questions during execution. This allows you to:
1. Gather user preferences or requirements
2. Clarify ambiguous instructions
3. Get decisions on implementation choices as you work
4. Offer choices to the user about what direction to take

Usage notes:
- Users will always be able to select "Other" to provide custom text input
- Use multiSelect: true to allow multiple answers to be selected for a question
- If you recommend a specific option, make that the first option in the list and add "(Recommended)" at the end of the label"#
            .to_string(),
        strict: false,
        parameters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_ask_user_question_tool_spec() {
        let spec = create_ask_user_question_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        assert_eq!(tool.name, names::ASK_USER_QUESTION);
        assert!(!tool.strict);
        assert!(tool.description.contains("ask the user questions"));
        assert!(tool.description.contains("multiSelect"));
    }

    #[test]
    fn test_questions_parameter() {
        let spec = create_ask_user_question_tool();

        let ToolSpec::Function(tool) = spec else {
            panic!("Expected function tool spec");
        };

        let JsonSchema::Object {
            properties,
            required,
            ..
        } = tool.parameters
        else {
            panic!("Expected object parameters");
        };

        // Check questions is required
        assert!(required.is_some());
        assert!(required.unwrap().contains(&"questions".to_string()));

        // Check questions is an array
        let questions = properties.get("questions").expect("questions property");
        assert!(matches!(questions, JsonSchema::Array { .. }));
    }
}
