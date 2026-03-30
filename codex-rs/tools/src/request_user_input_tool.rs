use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub fn create_request_user_input_tool(description: String) -> ToolSpec {
    let option_props = BTreeMap::from([
        (
            "label".to_string(),
            JsonSchema::String {
                description: Some("User-facing label (1-5 words).".to_string()),
            },
        ),
        (
            "description".to_string(),
            JsonSchema::String {
                description: Some(
                    "One short sentence explaining impact/tradeoff if selected.".to_string(),
                ),
            },
        ),
    ]);

    let options_schema = JsonSchema::Array {
        description: Some(
            "Provide 2-3 mutually exclusive choices. Put the recommended option first and suffix its label with \"(Recommended)\". Do not include an \"Other\" option in this list; the client will add a free-form \"Other\" option automatically."
                .to_string(),
        ),
        items: Box::new(JsonSchema::Object {
            properties: option_props,
            required: Some(vec!["label".to_string(), "description".to_string()]),
            additional_properties: Some(false.into()),
        }),
    };

    let question_props = BTreeMap::from([
        (
            "id".to_string(),
            JsonSchema::String {
                description: Some(
                    "Stable identifier for mapping answers (snake_case).".to_string(),
                ),
            },
        ),
        (
            "header".to_string(),
            JsonSchema::String {
                description: Some(
                    "Short header label shown in the UI (12 or fewer chars).".to_string(),
                ),
            },
        ),
        (
            "question".to_string(),
            JsonSchema::String {
                description: Some("Single-sentence prompt shown to the user.".to_string()),
            },
        ),
        ("options".to_string(), options_schema),
    ]);

    let questions_schema = JsonSchema::Array {
        description: Some("Questions to show the user. Prefer 1 and do not exceed 3".to_string()),
        items: Box::new(JsonSchema::Object {
            properties: question_props,
            required: Some(vec![
                "id".to_string(),
                "header".to_string(),
                "question".to_string(),
                "options".to_string(),
            ]),
            additional_properties: Some(false.into()),
        }),
    };

    let properties = BTreeMap::from([("questions".to_string(), questions_schema)]);

    ToolSpec::Function(ResponsesApiTool {
        name: "request_user_input".to_string(),
        description,
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::Object {
            properties,
            required: Some(vec!["questions".to_string()]),
            additional_properties: Some(false.into()),
        },
        output_schema: None,
    })
}

#[cfg(test)]
#[path = "request_user_input_tool_tests.rs"]
mod tests;
