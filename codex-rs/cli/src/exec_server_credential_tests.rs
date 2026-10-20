use std::io::PipeWriter;
use std::io::Write;

use pretty_assertions::assert_eq;

use super::*;

fn pipe() -> (PipeWriter, ExecutorCredentialPipe) {
    let (reader, writer) = std::io::pipe().expect("create credential pipe");
    let pipe = ExecutorCredentialPipe::spawn_with_reader("env-a".to_string(), move || reader);
    (writer, pipe)
}

fn frame(environment_id: &str, token: &str, expires_at: u64) -> Vec<u8> {
    let mut value = serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "environment_id": environment_id,
        "token": token,
        "expires_at": expires_at,
    }))
    .expect("serialize frame");
    value.push(b'\n');
    value
}

async fn next_update(updates: &mut watch::Receiver<CredentialState>) {
    tokio::time::timeout(Duration::from_secs(2), updates.changed())
        .await
        .expect("credential reader should publish")
        .expect("reader should remain open");
}

#[tokio::test]
async fn waits_for_initial_rotation_replaces_token_and_eof_clears_auth() {
    let (mut writer, pipe) = pipe();
    let provider = pipe.provider;
    let mut updates = provider.current.clone();
    let expires = unix_timestamp().expect("clock") + 60;
    assert_eq!(
        provider
            .wait_for_initial_with_timeout(Duration::from_millis(5))
            .await,
        Err(CredentialError::InitialTimeout)
    );
    assert!(provider.resolve_auth_headers().await.is_err());
    assert!(provider.to_auth_headers().is_err());
    writer
        .write_all(&frame("env-a", "first-scoped-token", expires))
        .expect("write");
    provider.wait_for_initial().await.expect("initial token");
    next_update(&mut updates).await;
    let headers = provider.resolve_auth_headers().await.expect("headers");
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[AUTHORIZATION], "Bearer first-scoped-token");
    assert!(headers[AUTHORIZATION].is_sensitive());
    assert!(!format!("{provider:?} {headers:?}").contains("first-scoped-token"));

    writer
        .write_all(&frame("env-a", "rotated-scoped-token", expires))
        .expect("rotate");
    next_update(&mut updates).await;
    let headers = provider.resolve_auth_headers().await.expect("headers");
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[AUTHORIZATION], "Bearer rotated-scoped-token");
    assert!(headers[AUTHORIZATION].is_sensitive());

    drop(writer);
    pipe.parent_closed.await.expect("reader must monitor EOF");
    assert!(provider.resolve_auth_headers().await.is_err());
    assert_eq!(provider.check_pipe_failure(), Ok(()));
}

#[tokio::test]
async fn malformed_rotations_latch_failure_and_clear_existing_credential() {
    let expires = unix_timestamp().expect("clock") + 60;
    let mut oversized = vec![b'x'; MAX_FRAME_BYTES];
    oversized.push(b'\n');
    for (bad_frame, error) in [
        (
            frame("env-b", "do-not-log-cross-env", expires),
            CredentialError::WrongEnvironment,
        ),
        (
            frame("env-a", "do-not-log-expired", /*expires_at*/ 1),
            CredentialError::Expired,
        ),
        (
            frame("env-a", "invalid\ntoken", expires),
            CredentialError::InvalidFrame,
        ),
        (
            b"invalid do-not-log-malformed\n".to_vec(),
            CredentialError::InvalidFrame,
        ),
        (b"\n".to_vec(), CredentialError::InvalidFrame),
        (oversized, CredentialError::FrameTooLarge),
    ] {
        let (mut writer, pipe) = pipe();
        writer
            .write_all(&frame("env-a", "initial-scoped-token", expires))
            .expect("initial frame");
        pipe.provider
            .wait_for_initial()
            .await
            .expect("initial credential");
        writer.write_all(&bad_frame).expect("bad frame");
        pipe.parent_closed
            .await
            .expect("bad frame terminates without EOF");
        let auth_error = pipe
            .provider
            .resolve_auth_headers()
            .await
            .expect_err("reject auth");
        assert_eq!(pipe.provider.check_pipe_failure(), Err(error));
        assert!(!auth_error.to_string().contains("do-not-log"));
        assert!(!auth_error.to_string().contains("initial-scoped-token"));
        let _ = writer.write_all(&frame("env-a", "later-scoped-token", expires));
        assert_eq!(pipe.provider.check_pipe_failure(), Err(error));
    }
}

#[tokio::test]
async fn eof_before_a_complete_frame_never_authenticates() {
    let expires = unix_timestamp().expect("clock") + 60;
    for (bytes, expected) in [
        (Vec::new(), CredentialError::Closed),
        (
            frame("env-a", "scoped-token", expires)
                .split_last()
                .expect("newline")
                .1
                .to_vec(),
            CredentialError::InvalidFrame,
        ),
    ] {
        let (mut writer, pipe) = pipe();
        if !bytes.is_empty() {
            writer.write_all(&bytes).expect("partial frame");
        }
        drop(writer);
        pipe.parent_closed.await.expect("reader monitors EOF");
        assert_eq!(pipe.provider.wait_for_initial().await, Err(expected));
        assert!(pipe.provider.resolve_auth_headers().await.is_err());
    }
}

#[tokio::test]
async fn expiry_is_checked_again_on_every_request() {
    let expired = unix_timestamp().expect("clock");
    let (updates, current) = watch::channel(CredentialState::Active {
        token: HeaderValue::from_static("unused-expired-token"),
        expires_at: expired,
    });
    let provider = ExecutorCredentialProvider { current };
    assert_eq!(
        provider.wait_for_initial().await,
        Err(CredentialError::Expired)
    );
    assert!(provider.resolve_auth_headers().await.is_err());
    updates.send_replace(CredentialState::Active {
        token: HeaderValue::from_static("new-scoped-token"),
        expires_at: expired + 60,
    });
    let headers = provider
        .resolve_auth_headers()
        .await
        .expect("rotation restores auth");
    assert_eq!(headers[AUTHORIZATION], "Bearer new-scoped-token");
}

#[test]
fn parser_rejects_unsupported_schema_without_exposing_input() {
    let expires = unix_timestamp().expect("clock") + 60;
    for invalid in [
        serde_json::json!({"version": 2, "environment_id": "env-a", "token": "secret", "expires_at": expires}),
        serde_json::json!({"version": 1, "environment_id": "env-a", "token": "secret", "expires_at": expires, "unexpected": true}),
        serde_json::json!({"version": 1, "environment_id": "env-a", "token": "secret", "expires_at": -1}),
        serde_json::json!({"version": 1, "environment_id": "env-a", "token": " ", "expires_at": expires}),
        serde_json::json!({"version": 1, "environment_id": "env-a", "token": "secret token", "expires_at": expires}),
        serde_json::json!({"version": 1, "environment_id": "env-a", "token": "secret-☃", "expires_at": expires}),
    ] {
        let serialized = serde_json::to_vec(&invalid).expect("serialize test input");
        let error = parse_frame(&serialized, "env-a")
            .err()
            .expect("invalid schema");
        assert_eq!(error, CredentialError::InvalidFrame);
        assert!(!error.to_string().contains("secret"));
    }
}
