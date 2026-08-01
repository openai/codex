use super::*;

use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn request_user_input_event_defaults_legacy_missing_is_blocking_to_true() {
    let event: RequestUserInputEvent = serde_json::from_value(json!({
        "call_id": "call-1",
        "turn_id": "turn-1",
        "questions": [{
            "id": "q1",
            "header": "Confirm",
            "question": "Continue?",
            "options": [{
                "label": "Yes",
                "description": "Continue."
            }]
        }],
        "autoResolutionMs": 60_000
    }))
    .expect("legacy request_user_input event should deserialize");

    assert_eq!(
        event,
        RequestUserInputEvent {
            call_id: "call-1".to_string(),
            turn_id: "turn-1".to_string(),
            questions: vec![RequestUserInputQuestion {
                id: "q1".to_string(),
                header: "Confirm".to_string(),
                question: "Continue?".to_string(),
                is_other: false,
                is_secret: false,
                options: Some(vec![RequestUserInputQuestionOption {
                    label: "Yes".to_string(),
                    description: "Continue.".to_string(),
                }]),
            }],
            is_blocking: true,
            auto_resolution_ms: Some(60_000),
        }
    );
}
