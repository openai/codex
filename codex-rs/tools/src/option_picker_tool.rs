use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use std::collections::BTreeMap;

pub const REQUEST_OPTION_PICKER_TOOL_NAME: &str = "request_option_picker";

pub fn create_request_option_picker_tool() -> ToolSpec {
    let option_props = BTreeMap::from([
        (
            "label".to_string(),
            JsonSchema::string(Some("User-facing option label.".to_string())),
        ),
        (
            "description".to_string(),
            JsonSchema::string(Some(
                "Optional short description for the option.".to_string(),
            )),
        ),
    ]);
    let properties = BTreeMap::from([
        (
            "question".to_string(),
            JsonSchema::string(Some("Question to show the user.".to_string())),
        ),
        (
            "options".to_string(),
            JsonSchema::array(
                JsonSchema::object(
                    option_props,
                    Some(vec!["label".to_string()]),
                    Some(false.into()),
                ),
                Some("Selectable options to show in the picker.".to_string()),
            ),
        ),
        (
            "allowMultiple".to_string(),
            JsonSchema::boolean(Some(
                "Set true when the user may choose more than one option.".to_string(),
            )),
        ),
        (
            "submitLabel".to_string(),
            JsonSchema::string(Some("Optional label for the submit button.".to_string())),
        ),
        (
            "skipLabel".to_string(),
            JsonSchema::string(Some("Optional label for the skip button.".to_string())),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: REQUEST_OPTION_PICKER_TOOL_NAME.to_string(),
        description: "Ask the user to choose from a compact list of onboarding options. Use only when setting up Codex from the onboarding tutorial.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["question".to_string(), "options".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}
