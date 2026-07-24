use std::time::Duration;

use pretty_assertions::assert_eq;

use super::*;
use crate::protocol::ExecOutputStream;
use crate::protocol::ProcessOutputChunk;

fn registry_error(status: http::StatusCode, code: Option<&str>) -> ExecServerError {
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
    let error = registry_error(http::StatusCode::TOO_MANY_REQUESTS, /*code*/ None);

    assert!(is_retryable_registry_error(&error));
    assert!(is_retryable_recovery_error(&error));
}

#[test]
fn recovery_retries_environment_offline_conflicts() {
    let error = registry_error(http::StatusCode::CONFLICT, Some("environment_offline"));

    assert!(is_retryable_registry_error(&error));
    assert!(is_retryable_recovery_error(&error));
}

#[test]
fn recovery_does_not_retry_other_registry_conflicts() {
    let error = registry_error(http::StatusCode::CONFLICT, Some("registration_conflict"));

    assert!(!is_retryable_registry_error(&error));
    assert!(!is_retryable_recovery_error(&error));
}

#[test]
fn process_event_reorder_rejects_oversized_output() {
    let state = SessionState::new(/*recoverable*/ true);

    let error = state
        .publish_ordered_event(ExecProcessEvent::Output(ProcessOutputChunk {
            seq: 1,
            stream: ExecOutputStream::Stdout,
            chunk: vec![0; super::super::MAX_PENDING_PROCESS_EVENT_BYTES + 1].into(),
        }))
        .expect_err("oversized pending process output should be rejected");

    assert!(error.contains("bytes"));
}

#[test]
fn process_event_reorder_accepts_gap_closing_event_at_limits() {
    let state = SessionState::new(/*recoverable*/ true);
    let chunk_size =
        super::super::MAX_PENDING_PROCESS_EVENT_BYTES / super::super::MAX_PENDING_PROCESS_EVENTS;
    let last_seq = super::super::MAX_PENDING_PROCESS_EVENTS as u64 + 1;

    for seq in 2..=last_seq {
        assert!(
            !state
                .publish_ordered_event(ExecProcessEvent::Output(ProcessOutputChunk {
                    seq,
                    stream: ExecOutputStream::Stdout,
                    chunk: vec![0; chunk_size].into(),
                }))
                .expect("future output should fit within reorder limits")
        );
    }
    assert!(
        !state
            .publish_ordered_event(ExecProcessEvent::Output(ProcessOutputChunk {
                seq: 1,
                stream: ExecOutputStream::Stdout,
                chunk: b"x".to_vec().into(),
            }))
            .expect("gap-closing output should drain the reorder buffer")
    );

    let ordered_events = state
        .ordered_events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        (
            ordered_events.last_published_seq,
            ordered_events.pending.len(),
            ordered_events.pending_bytes,
        ),
        (last_seq, 0, 0)
    );
}

#[test]
fn recovery_handles_dense_tail_output_and_newer_notification() {
    let state = SessionState::new(/*recoverable*/ true);
    let last_seq = super::super::MAX_PENDING_PROCESS_EVENTS as u64 + 2;
    let live_seq = last_seq + 1;
    assert!(
        !state
            .publish_ordered_event(ExecProcessEvent::Output(ProcessOutputChunk {
                seq: live_seq,
                stream: ExecOutputStream::Stdout,
                chunk: b"live".to_vec().into(),
            }))
            .expect("live output should remain bounded while recovery fills the gap")
    );
    let chunks = (2..=last_seq)
        .map(|seq| ProcessOutputChunk {
            seq,
            stream: ExecOutputStream::Stdout,
            chunk: b"x".to_vec().into(),
        })
        .collect();

    assert!(
        !state
            .recover_events(ReadResponse {
                chunks,
                next_seq: last_seq + 1,
                exited: true,
                exit_code: Some(17),
                closed: false,
                failure: None,
                sandbox_denied: false,
            })
            .expect("dense retained output should recover")
    );

    let ordered_events = state
        .ordered_events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        (
            ordered_events.last_published_seq,
            ordered_events.pending.len(),
            ordered_events.pending_bytes,
        ),
        (live_seq, 0, 0)
    );
}

#[test]
fn recovery_rejects_output_at_closed_sequence() {
    let state = SessionState::new(/*recoverable*/ true);

    let error = state
        .recover_events(ReadResponse {
            chunks: vec![ProcessOutputChunk {
                seq: 1,
                stream: ExecOutputStream::Stdout,
                chunk: b"output".to_vec().into(),
            }],
            next_seq: 2,
            exited: false,
            exit_code: None,
            closed: true,
            failure: None,
            sandbox_denied: false,
        })
        .expect_err("output should not occupy the closed sequence");

    assert!(
        error
            .to_string()
            .contains("conflicts with recovered output")
    );
}

#[tokio::test]
async fn recovery_adds_sandbox_denial_to_pending_exit_event() {
    let state = SessionState::new(/*recoverable*/ true);
    assert!(
        !state
            .publish_ordered_event(ExecProcessEvent::Exited {
                seq: 2,
                exit_code: 1,
                sandbox_denied: None,
            })
            .expect("pending exit should fit within reorder limits")
    );

    state
        .recover_events(ReadResponse {
            chunks: vec![ProcessOutputChunk {
                seq: 1,
                stream: ExecOutputStream::Stderr,
                chunk: b"sandbox denied".to_vec().into(),
            }],
            next_seq: 3,
            exited: true,
            exit_code: Some(1),
            closed: false,
            failure: None,
            sandbox_denied: true,
        })
        .expect("recovery should publish the pending exit");

    let mut events = state.subscribe_events();
    assert!(matches!(
        events.recv().await,
        Ok(ExecProcessEvent::Output(_))
    ));
    assert_eq!(
        events.recv().await,
        Ok(ExecProcessEvent::Exited {
            seq: 2,
            exit_code: 1,
            sandbox_denied: Some(true),
        })
    );
}
