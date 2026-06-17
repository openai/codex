use std::time::Duration;

use super::*;

fn registry_error() -> ExecServerError {
    ExecServerError::EnvironmentRegistryHttp {
        status: reqwest::StatusCode::TOO_MANY_REQUESTS,
        code: None,
        message: "registry unavailable".to_string(),
    }
}

#[test]
fn registry_recovery_retry_delay_exponentially_backs_off_and_caps() {
    let cases = [
        (0, Duration::from_millis(500)),
        (1, Duration::from_secs(1)),
        (2, Duration::from_secs(2)),
        (3, Duration::from_secs(4)),
        (4, Duration::from_secs(5)),
        (20, Duration::from_secs(5)),
    ];

    for (attempt, maximum) in cases {
        let delay = registry_recovery_retry_delay("session-1", attempt);
        assert!(
            delay >= maximum / 2,
            "delay {delay:?} for attempt {attempt}"
        );
        assert!(delay <= maximum, "delay {delay:?} for attempt {attempt}");
    }
}

#[test]
fn recovery_retries_transient_registry_errors() {
    let error = registry_error();

    assert!(is_retryable_registry_error(&error));
    assert!(is_retryable_recovery_error(&error));
}
