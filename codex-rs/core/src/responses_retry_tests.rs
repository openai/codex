use super::*;
use pretty_assertions::assert_eq;

#[test]
fn detects_unsupported_reasoning_effort_errors() {
    let structured = CodexErr::InvalidRequest(
        serde_json::json!({
            "error": {
                "message": "Invalid value: 'max'. Supported values are: 'low', 'medium', and 'high'.",
                "param": "reasoning.effort"
            }
        })
        .to_string(),
    );
    let message_only = CodexErr::InvalidRequest(
        "Invalid value: 'max'. Supported values are: 'low', 'medium', and 'high'.".to_string(),
    );
    let unrelated = CodexErr::InvalidRequest("Invalid value: 'other'.".to_string());

    assert_eq!(
        [
            is_unsupported_reasoning_effort_error(&structured, &ReasoningEffort::Ultra),
            is_unsupported_reasoning_effort_error(
                &message_only,
                &ReasoningEffort::Custom("max".to_string()),
            ),
            is_unsupported_reasoning_effort_error(&unrelated, &ReasoningEffort::Ultra),
        ],
        [true, true, false]
    );
}
