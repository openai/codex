use super::*;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn request_user_input_tool_includes_questions_schema() {
    assert_eq!(
        create_request_user_input_tool("Ask the user to choose.".to_string()),
        ToolSpec::Function(ResponsesApiTool {
            name: "request_user_input".to_string(),
            description: "Ask the user to choose.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::Object {
                properties: BTreeMap::from([(
                    "questions".to_string(),
                    JsonSchema::Array {
                        description: Some(
                            "Questions to show the user. Prefer 1 and do not exceed 3".to_string(),
                        ),
                        items: Box::new(JsonSchema::Object {
                            properties: BTreeMap::from([
                                (
                                    "header".to_string(),
                                    JsonSchema::String {
                                        description: Some(
                                            "Short header label shown in the UI (12 or fewer chars)."
                                                .to_string(),
                                        ),
                                    },
                                ),
                                (
                                    "id".to_string(),
                                    JsonSchema::String {
                                        description: Some(
                                            "Stable identifier for mapping answers (snake_case)."
                                                .to_string(),
                                        ),
                                    },
                                ),
                                (
                                    "options".to_string(),
                                    JsonSchema::Array {
                                        description: Some(
                                            "Provide 2-3 mutually exclusive choices. Put the recommended option first and suffix its label with \"(Recommended)\". Do not include an \"Other\" option in this list; the client will add a free-form \"Other\" option automatically."
                                                .to_string(),
                                        ),
                                        items: Box::new(JsonSchema::Object {
                                            properties: BTreeMap::from([
                                                (
                                                    "description".to_string(),
                                                    JsonSchema::String {
                                                        description: Some(
                                                            "One short sentence explaining impact/tradeoff if selected."
                                                                .to_string(),
                                                        ),
                                                    },
                                                ),
                                                (
                                                    "label".to_string(),
                                                    JsonSchema::String {
                                                        description: Some(
                                                            "User-facing label (1-5 words)."
                                                                .to_string(),
                                                        ),
                                                    },
                                                ),
                                            ]),
                                            required: Some(vec![
                                                "label".to_string(),
                                                "description".to_string(),
                                            ]),
                                            additional_properties: Some(false.into()),
                                        }),
                                    },
                                ),
                                (
                                    "question".to_string(),
                                    JsonSchema::String {
                                        description: Some(
                                            "Single-sentence prompt shown to the user.".to_string(),
                                        ),
                                    },
                                ),
                            ]),
                            required: Some(vec![
                                "id".to_string(),
                                "header".to_string(),
                                "question".to_string(),
                                "options".to_string(),
                            ]),
                            additional_properties: Some(false.into()),
                        }),
                    },
                )]),
                required: Some(vec!["questions".to_string()]),
                additional_properties: Some(false.into()),
            },
            output_schema: None,
        })
    );
}
