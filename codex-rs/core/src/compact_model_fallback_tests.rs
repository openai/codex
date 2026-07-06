use super::is_model_unavailable_error;
use codex_protocol::error::CodexErr;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn model_unavailable_error_requires_the_expected_model_rejection() {
    let model = "gpt-5.6-oai";
    let expected_message =
        "The 'gpt-5.6-oai' model is not supported when using Codex with a ChatGPT account.";
    let errors = [
        CodexErr::InvalidRequest(expected_message.to_string()),
        CodexErr::InvalidRequest(
            json!({
                "error": {
                    "type": "invalid_request_error",
                    "message": expected_message,
                }
            })
            .to_string(),
        ),
        CodexErr::InvalidRequest(
            "The 'gpt-5.7-oai' model is not supported when using Codex with a ChatGPT account."
                .to_string(),
        ),
        CodexErr::InvalidRequest("generic invalid request".to_string()),
        CodexErr::RequestTimeout,
    ];

    assert_eq!(
        errors
            .iter()
            .map(|error| is_model_unavailable_error(error, model))
            .collect::<Vec<_>>(),
        vec![true, true, false, false, false]
    );
}
