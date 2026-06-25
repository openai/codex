use std::time::Duration;

use super::*;

fn registry_error(status: reqwest::StatusCode, code: Option<&str>) -> ExecServerError {
    ExecServerError::EnvironmentRegistryHttp {
        status,
        code: code.map(str::to_string),
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

    for (attempt, base) in cases {
        let delay = registry_recovery_retry_delay("session-1", attempt);
        assert!(delay >= base, "delay {delay:?} for attempt {attempt}");
        assert!(
            delay <= base + base / 2,
            "delay {delay:?} for attempt {attempt}"
        );
    }
}

#[test]
fn recovery_retries_transient_registry_errors() {
    let error = registry_error(reqwest::StatusCode::TOO_MANY_REQUESTS, /*code*/ None);

    assert!(is_retryable_registry_error(&error));
    assert!(is_retryable_recovery_error(&error));
}

#[test]
fn recovery_retries_environment_offline_conflicts() {
    let error = registry_error(reqwest::StatusCode::CONFLICT, Some("environment_offline"));

    assert!(is_retryable_registry_error(&error));
    assert!(is_retryable_recovery_error(&error));
}

#[test]
fn recovery_does_not_retry_other_registry_conflicts() {
    let error = registry_error(reqwest::StatusCode::CONFLICT, Some("registration_conflict"));

    assert!(!is_retryable_registry_error(&error));
    assert!(!is_retryable_recovery_error(&error));
}
