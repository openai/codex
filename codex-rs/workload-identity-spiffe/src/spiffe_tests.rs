use super::*;

#[cfg(unix)]
use std::future::pending;
#[cfg(unix)]
use tokio::net::UnixListener;

#[tokio::test]
async fn rejects_invalid_explicit_spiffe_id_before_connecting() {
    let source = SpiffeSubjectTokenProvider::new(
        Some("unix:/tmp/does-not-exist.sock".to_string()),
        Some("not-a-spiffe-id".to_string()),
        "openai-audience".to_string(),
    );

    assert!(matches!(
        source.subject_token().await,
        Err(SubjectTokenError::InvalidConfiguration { provider: "spiffe" })
    ));
}

#[tokio::test]
async fn rejects_tcp_endpoint() {
    let source = SpiffeSubjectTokenProvider::new(
        Some("tcp://127.0.0.1:8081".to_string()),
        /*spiffe_id*/ None,
        "openai-audience".to_string(),
    );

    assert!(matches!(
        source.subject_token().await,
        Err(SubjectTokenError::InvalidConfiguration { provider: "spiffe" })
    ));
}

#[cfg(unix)]
#[tokio::test]
async fn times_out_when_workload_api_stalls() {
    let temp_dir = tempfile::tempdir().expect("create temporary SPIFFE socket directory");
    let socket_path = temp_dir.path().join("workload-api.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind stalled Workload API socket");
    let stalled_server = tokio::spawn(async move {
        let _accepted = listener.accept().await.expect("accept Workload API client");
        pending::<()>().await;
    });
    let source = SpiffeSubjectTokenProvider::new(
        Some(format!("unix:{}", socket_path.display())),
        /*spiffe_id*/ None,
        "openai-audience".to_string(),
    );

    let result = tokio::time::timeout(Duration::from_secs(2), source.subject_token())
        .await
        .expect("provider should enforce its own timeout");
    stalled_server.abort();

    assert!(matches!(
        result,
        Err(SubjectTokenError::Unavailable { provider: "spiffe" })
    ));
}
