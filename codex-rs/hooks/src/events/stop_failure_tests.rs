use codex_protocol::error::CodexErr;
use codex_protocol::error::UsageLimitReachedError;
use codex_protocol::protocol::RateLimitReachedType;
use pretty_assertions::assert_eq;

use super::StopFailureError;
use super::StopFailureModelSelector;
use super::StopFailureOutput;
use super::StopFailureRecovery;
use crate::engine::output_parser::parse_stop_failure;

fn usage_limit_error(rate_limit_reached_type: RateLimitReachedType) -> CodexErr {
    CodexErr::UsageLimitReached(UsageLimitReachedError {
        plan_type: None,
        resets_at: None,
        rate_limits: None,
        promo_message: None,
        rate_limit_reached_type: Some(rate_limit_reached_type),
    })
}

#[test]
fn classifies_recoverable_api_errors() {
    let cases = [
        (CodexErr::ServerOverloaded, StopFailureError::Overloaded),
        (
            usage_limit_error(RateLimitReachedType::RateLimitReached),
            StopFailureError::RateLimit,
        ),
        (
            usage_limit_error(RateLimitReachedType::WorkspaceOwnerCreditsDepleted),
            StopFailureError::BillingError,
        ),
        (
            CodexErr::InvalidRequest("bad request".to_string()),
            StopFailureError::InvalidRequest,
        ),
        (CodexErr::InternalServerError, StopFailureError::ServerError),
    ];

    for (error, expected) in cases {
        assert_eq!(StopFailureError::classify(&error), Some(expected));
    }

    assert_eq!(StopFailureError::classify(&CodexErr::TurnAborted), None);
}

#[test]
fn parses_each_model_selector() {
    let cases = [
        (
            serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "StopFailure",
                    "recovery": { "action": "retry" }
                }
            }),
            StopFailureModelSelector::Current,
        ),
        (
            serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "StopFailure",
                    "recovery": {
                        "action": "retry",
                        "model": { "selector": "catalog_default" }
                    }
                }
            }),
            StopFailureModelSelector::CatalogDefault,
        ),
        (
            serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "StopFailure",
                    "recovery": {
                        "action": "retry",
                        "model": { "selector": "id", "id": "gpt-5.4" }
                    }
                }
            }),
            StopFailureModelSelector::Id("gpt-5.4".to_string()),
        ),
    ];

    for (output, expected_model) in cases {
        assert_eq!(
            parse_stop_failure(&output.to_string()),
            Some(StopFailureOutput {
                recovery: Some(StopFailureRecovery {
                    model: expected_model,
                    reason: None,
                }),
            })
        );
    }
}

#[test]
fn absent_recovery_is_a_valid_noop() {
    assert_eq!(
        parse_stop_failure("{}"),
        Some(StopFailureOutput { recovery: None })
    );
}

#[test]
fn empty_explicit_model_id_is_rejected() {
    let output = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "StopFailure",
            "recovery": {
                "action": "retry",
                "model": { "selector": "id", "id": "  " }
            }
        }
    });

    assert_eq!(parse_stop_failure(&output.to_string()), None);
}
