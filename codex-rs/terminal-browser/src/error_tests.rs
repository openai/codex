use pretty_assertions::assert_eq;
use serde_json::json;

use super::BrowserErrorCode;
use super::classify_browser_error;

#[test]
fn model_facing_failures_are_structured_and_sanitized() {
    let error = anyhow::anyhow!("CDP Runtime.evaluate failed with secret=do-not-leak");
    let failure = classify_browser_error(&error);

    assert_eq!(
        serde_json::to_value(failure).expect("serialize failure"),
        json!({
            "code": "internal",
            "message": "the terminal-browser action failed; inspect local logs for details",
            "retryable": false,
        })
    );
}

#[test]
fn busy_errors_are_retryable() {
    let failure = classify_browser_error(&anyhow::anyhow!("browser_busy: already running"));
    assert_eq!(failure.code, BrowserErrorCode::BrowserBusy);
    assert!(failure.retryable);
}

#[test]
fn policy_and_stale_node_errors_use_actionable_codes() {
    let policy = classify_browser_error(&anyhow::anyhow!(
        "browser navigation blocked by the active permission policy"
    ));
    let stale = classify_browser_error(&anyhow::anyhow!("CDP: Could not find node with id 7"));

    assert_eq!(policy.code, BrowserErrorCode::PolicyChanged);
    assert_eq!(stale.code, BrowserErrorCode::StaleHandle);
}
