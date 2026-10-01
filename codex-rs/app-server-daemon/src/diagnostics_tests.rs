use super::Failure;
use pretty_assertions::assert_eq;

#[test]
fn failure_omits_error_messages_and_context() {
    let error = anyhow::Error::new(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "secret child output",
    ))
    .context("secret protocol data");
    assert_eq!(
        Failure::from(&error),
        Failure {
            kind: "PermissionDenied".to_string(),
            os_code: None
        },
    );
    let error = anyhow::anyhow!("secret child output");
    assert_eq!(
        Failure::from(&error),
        Failure {
            kind: "other".to_string(),
            os_code: None
        },
    );
}

#[test]
fn result_record_preserves_json_shape_and_redacts_error_text() {
    let error = anyhow::Error::new(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "secret child output",
    ));
    let record = super::Record {
        timestamp_ms: 123,
        process_pid: 456,
        payload: super::Payload::<()>::Result {
            stage: "process_spawn",
            outcome: super::Outcome::Failed,
            elapsed_ms: 789,
            failure: Some(Failure::from(&error)),
        },
    };
    assert_eq!(
        serde_json::to_value(record).unwrap(),
        serde_json::json!({
            "timestampMs": 123,
            "processPid": 456,
            "stage": "process_spawn",
            "outcome": "failed",
            "elapsedMs": 789,
            "failure": { "kind": "PermissionDenied", "os_code": null },
        }),
    );
}
