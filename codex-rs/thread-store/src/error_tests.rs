//! Verifies conservative classification at the shared storage error boundary.

use super::ThreadStoreError;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;

#[test]
fn unclassified_sqlx_failures_remain_internal() {
    for error in [
        sqlx::Error::PoolTimedOut,
        sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "connection lost after write",
        )),
        sqlx::Error::Protocol("retryable conflict; outcome unknown".to_owned()),
    ] {
        let expected = format!("failed to access thread history: {error}");
        let error = ThreadStoreError::from(error);
        let ThreadStoreError::Internal { message } = error else {
            panic!("unclassified SQLx failure must remain internal: {error:?}");
        };
        assert_eq!(message, expected);
    }
}

#[test]
fn missing_read_requires_the_requested_thread_and_a_known_store_error() {
    let requested = ThreadId::new();
    let other = ThreadId::new();
    let missing_rollout = format!("no rollout found for thread id {requested}");
    for error in [
        ThreadStoreError::ThreadNotFound {
            thread_id: requested,
        },
        ThreadStoreError::InvalidRequest {
            message: missing_rollout.clone(),
        },
    ] {
        assert!(error.is_missing_for_read(requested));
        assert!(!error.is_missing_for_read(other));
    }
    for error in [
        ThreadStoreError::InvalidRequest {
            message: format!("{missing_rollout} (lookup failed)"),
        },
        ThreadStoreError::InvalidRequest {
            message: format!("thread {requested} is archived"),
        },
        ThreadStoreError::Retryable {
            message: missing_rollout.clone(),
        },
        ThreadStoreError::Internal {
            message: missing_rollout,
        },
        ThreadStoreError::Unsupported {
            operation: "read_thread",
        },
    ] {
        assert!(!error.is_missing_for_read(requested), "{error:?}");
    }
}
