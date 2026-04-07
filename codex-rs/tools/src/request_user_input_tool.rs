use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::TUI_VISIBLE_COLLABORATION_MODES;
use codex_protocol::request_user_input::RequestUserInputArgs;
use std::collections::BTreeMap;

pub const REQUEST_USER_INPUT_TOOL_NAME: &str = "request_user_input";

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
        name: REQUEST_USER_INPUT_TOOL_NAME.to_string(),
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

pub fn request_user_input_unavailable_message(
    mode: ModeKind,
    default_mode_request_user_input: bool,
) -> Option<String> {
    if request_user_input_is_available(mode, default_mode_request_user_input) {
        None
    } else {
        let mode_name = mode.display_name();
        Some(format!(
            "request_user_input is unavailable in {mode_name} mode"
        ))
    }
}

pub fn normalize_request_user_input_args(
    mut args: RequestUserInputArgs,
) -> Result<RequestUserInputArgs, String> {
    let missing_options = args
        .questions
        .iter()
        .any(|question| question.options.as_ref().is_none_or(Vec::is_empty));
    if missing_options {
        return Err("request_user_input requires non-empty options for every question".to_string());
    }

    for question in &mut args.questions {
        question.is_other = true;
    }

    Ok(args)
}

pub fn request_user_input_tool_description(default_mode_request_user_input: bool) -> String {
    let allowed_modes = format_allowed_modes(default_mode_request_user_input);
    format!(
        "Request user input for one to three short questions and wait for the response. This tool is only available in {allowed_modes}."
    )
}

fn request_user_input_is_available(mode: ModeKind, default_mode_request_user_input: bool) -> bool {
    mode.allows_request_user_input()
        || (default_mode_request_user_input && mode == ModeKind::Default)
}

fn format_allowed_modes(default_mode_request_user_input: bool) -> String {
    let mode_names: Vec<&str> = TUI_VISIBLE_COLLABORATION_MODES
        .into_iter()
        .filter(|mode| request_user_input_is_available(*mode, default_mode_request_user_input))
        .map(ModeKind::display_name)
        .collect();

    match mode_names.as_slice() {
        [] => "no modes".to_string(),
        [mode] => format!("{mode} mode"),
        [first, second] => format!("{first} or {second} mode"),
        [..] => format!("modes: {}", mode_names.join(",")),
    }
}

#[cfg(test)]
#[path = "request_user_input_tool_tests.rs"]
mod tests;
