use super::*;
use codex_features::Feature;
use codex_features::Features;
use codex_protocol::config_types::ModeKind;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_tools::JsonSchema;
use codex_tools::request_user_input_available_modes;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

fn default_mode_enabled_available_modes() -> Vec<ModeKind> {
    let mut features = Features::with_defaults();
    features.enable(Feature::DefaultModeRequestUserInput);
    request_user_input_available_modes(&features)
}

fn default_available_modes() -> Vec<ModeKind> {
    request_user_input_available_modes(&Features::with_defaults())
}

#[test]
fn request_user_input_tool_includes_questions_schema() {
    assert_eq!(
        create_request_user_input_tool("Ask the user to choose.".to_string()),
        ToolSpec::Function(ResponsesApiTool {
            name: "request_user_input".to_string(),
            description: "Ask the user to choose.".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(BTreeMap::from([
                (
                    "autoResolutionMs".to_string(),
                    JsonSchema::number(Some(
                        "Optional auto-resolution window in milliseconds, from 60000 to 240000. Include this only when the question is useful but non-blocking and continuing with best judgment is acceptable if the user does not answer; omit it when explicit user input is required before continuing. Use 60000 for lightly helpful context and up to 240000 when the answer would materially unblock better work."
                            .to_string(),
                    )),
                ),
                (
                    "questions".to_string(),
                    JsonSchema::array(
                        JsonSchema::object(
                            BTreeMap::from([
                                (
                                    "header".to_string(),
                                    JsonSchema::string(Some(
                                        "Short header label shown in the UI (12 or fewer chars)."
                                            .to_string(),
                                    )),
                                ),
                                (
                                    "id".to_string(),
                                    JsonSchema::string(Some(
                                        "Stable identifier for mapping answers (snake_case)."
                                            .to_string(),
                                    )),
                                ),
                                (
                                    "options".to_string(),
                                    JsonSchema::array(
                                        JsonSchema::object(
                                            BTreeMap::from([
                                                (
                                                    "description".to_string(),
                                                    JsonSchema::string(Some(
                                                        "One short sentence explaining impact/tradeoff if selected."
                                                            .to_string(),
                                                    )),
                                                ),
                                                (
                                                    "label".to_string(),
                                                    JsonSchema::string(Some(
                                                        "User-facing label (1-5 words)."
                                                            .to_string(),
                                                    )),
                                                ),
                                            ]),
                                            Some(vec![
                                                "label".to_string(),
                                                "description".to_string(),
                                            ]),
                                            Some(false.into()),
                                        ),
                                        Some(
                                            "Provide 2-3 mutually exclusive choices. Put the recommended option first and suffix its label with \"(Recommended)\". Do not include an \"Other\" option in this list; the client will add a free-form \"Other\" option automatically."
                                                .to_string(),
                                        ),
                                    ),
                                ),
                                (
                                    "question".to_string(),
                                    JsonSchema::string(Some(
                                        "Single-sentence prompt shown to the user.".to_string(),
                                    )),
                                ),
                            ]),
                            Some(vec![
                                "id".to_string(),
                                "header".to_string(),
                                "question".to_string(),
                                "options".to_string(),
                            ]),
                            Some(false.into()),
                        ),
                        Some(
                            "Questions to show the user. Prefer 1 and do not exceed 3".to_string(),
                        ),
                    ),
                ),
            ]), Some(vec!["questions".to_string()]), Some(false.into())),
            output_schema: None,
        })
    );
}

#[test]
fn normalize_request_user_input_args_clamps_out_of_range_auto_resolution_ms() {
    let args = RequestUserInputArgs {
        questions: vec![RequestUserInputQuestion {
            id: "confirm".to_string(),
            header: "Confirm".to_string(),
            question: "Proceed?".to_string(),
            is_other: false,
            is_secret: false,
            options: Some(vec![RequestUserInputQuestionOption {
                label: "Yes (Recommended)".to_string(),
                description: "Continue.".to_string(),
            }]),
        }],
        auto_resolution_ms: Some(MIN_AUTO_RESOLUTION_MS - 1),
    };

    assert_eq!(
        normalize_request_user_input_args(args.clone()),
        Ok(RequestUserInputArgs {
            questions: vec![RequestUserInputQuestion {
                is_other: true,
                ..args.questions[0].clone()
            }],
            auto_resolution_ms: Some(MIN_AUTO_RESOLUTION_MS),
        })
    );
    assert_eq!(
        normalize_request_user_input_args(RequestUserInputArgs {
            auto_resolution_ms: Some(MAX_AUTO_RESOLUTION_MS + 1),
            ..args.clone()
        }),
        Ok(RequestUserInputArgs {
            questions: vec![RequestUserInputQuestion {
                is_other: true,
                ..args.questions[0].clone()
            }],
            auto_resolution_ms: Some(MAX_AUTO_RESOLUTION_MS),
        })
    );
}

#[test]
fn normalize_request_user_input_args_accepts_auto_resolution_boundaries() {
    let args = RequestUserInputArgs {
        questions: vec![RequestUserInputQuestion {
            id: "confirm".to_string(),
            header: "Confirm".to_string(),
            question: "Proceed?".to_string(),
            is_other: false,
            is_secret: false,
            options: Some(vec![RequestUserInputQuestionOption {
                label: "Yes (Recommended)".to_string(),
                description: "Continue.".to_string(),
            }]),
        }],
        auto_resolution_ms: Some(MIN_AUTO_RESOLUTION_MS),
    };

    assert_eq!(
        normalize_request_user_input_args(args.clone()),
        Ok(RequestUserInputArgs {
            questions: vec![RequestUserInputQuestion {
                is_other: true,
                ..args.questions[0].clone()
            }],
            auto_resolution_ms: Some(MIN_AUTO_RESOLUTION_MS),
        })
    );
    assert_eq!(
        normalize_request_user_input_args(RequestUserInputArgs {
            auto_resolution_ms: Some(MAX_AUTO_RESOLUTION_MS),
            ..args.clone()
        }),
        Ok(RequestUserInputArgs {
            questions: vec![RequestUserInputQuestion {
                is_other: true,
                ..args.questions[0].clone()
            }],
            auto_resolution_ms: Some(MAX_AUTO_RESOLUTION_MS),
        })
    );
}

#[test]
fn request_user_input_unavailable_messages_respect_default_mode_feature_flag() {
    assert_eq!(
        request_user_input_unavailable_message(ModeKind::Plan, &default_available_modes()),
        None
    );
    assert_eq!(
        request_user_input_unavailable_message(ModeKind::Default, &default_available_modes()),
        Some("request_user_input is unavailable in Default mode".to_string())
    );
    assert_eq!(
        request_user_input_unavailable_message(
            ModeKind::Default,
            &default_mode_enabled_available_modes()
        ),
        None
    );
    assert_eq!(
        request_user_input_unavailable_message(ModeKind::Execute, &default_available_modes()),
        Some("request_user_input is unavailable in Execute mode".to_string())
    );
    assert_eq!(
        request_user_input_unavailable_message(
            ModeKind::PairProgramming,
            &default_available_modes()
        ),
        Some("request_user_input is unavailable in Pair Programming mode".to_string())
    );
}

#[test]
fn request_user_input_tool_description_mentions_available_modes() {
    assert_eq!(
        request_user_input_tool_description(&default_available_modes()),
        "Request user input for one to three short questions and wait for the response. Set autoResolutionMs, from 60000 to 240000 milliseconds, only when the question is useful but non-blocking and continuing with best judgment is acceptable if the user does not answer; omit it when explicit user input is required. This tool is only available in Plan mode.".to_string()
    );
    assert_eq!(
        request_user_input_tool_description(&default_mode_enabled_available_modes()),
        "Request user input for one to three short questions and wait for the response. Set autoResolutionMs, from 60000 to 240000 milliseconds, only when the question is useful but non-blocking and continuing with best judgment is acceptable if the user does not answer; omit it when explicit user input is required. This tool is only available in Default or Plan mode.".to_string()
    );
}
