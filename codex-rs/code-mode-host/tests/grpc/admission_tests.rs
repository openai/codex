//! Initial admission may retry transient failures; established work must not replay.
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use pretty_assertions::assert_eq;

use super::*;

#[tokio::test]
async fn session_admission_retries_only_transient_failures_and_is_bounded() -> Result<()> {
    for (code, failures, expected_attempts, succeeds) in [
        (Code::Unavailable, 1, 2, true),
        (Code::ResourceExhausted, 1, 2, true),
        (Code::Unavailable, usize::MAX, 3, false),
        (Code::InvalidArgument, 1, 1, false),
        (Code::Unknown, 1, 1, false),
    ] {
        let attempts = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&attempts);
        let app = axum::Router::new()
            .fallback_service(grpc::code_mode_host_server::CodeModeHostServer::new(
                codex_code_mode_host::GrpcCodeModeHost::new(),
            ))
            .layer(axum::middleware::from_fn(
                move |request: axum::extract::Request, next: axum::middleware::Next| {
                    let count = Arc::clone(&count);
                    async move {
                        if request.uri().path().ends_with("/OpenSession")
                            && count.fetch_add(1, Ordering::SeqCst) < failures
                        {
                            return tonic::Status::new(code, "transient admission test")
                                .into_http::<axum::body::Body>();
                        }
                        next.run(request).await
                    }
                },
            ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("admission test server");
        }));
        let provider = GrpcCodeModeSessionProvider::new(format!("http://{address}"));
        let result = timeout(TEST_TIMEOUT, provider.create_session()).await?;
        assert_eq!(
            result.is_ok(),
            succeeds,
            "status {code}: {}",
            result.as_ref().err().cloned().unwrap_or_default()
        );
        assert_eq!(attempts.load(Ordering::SeqCst), expected_attempts);
        if let Ok(session) = result {
            let response = execute(
                &session,
                request("text(42);"),
                Arc::new(NoopCodeModeSessionDelegate),
            )
            .await?;
            assert_eq!(
                response,
                text_response("1", "42", response.code_mode_host_duration())
            );
            session.shutdown().await.map_err(anyhow::Error::msg)?;
        }
        drop(server);
    }
    Ok(())
}

#[tokio::test]
async fn session_admission_recovers_after_connection_reset() -> Result<()> {
    use tokio::io::AsyncReadExt;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let server = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        // Drop the first connection with unread HTTP/2 bytes to send TCP RST.
        // Serve the next connection normally; no timer or launcher mock is needed.
        let (mut stream, _) = listener.accept().await?;
        stream.read_exact(&mut [0]).await?;
        drop(stream);
        let app = axum::Router::new().fallback_service(
            grpc::code_mode_host_server::CodeModeHostServer::new(
                codex_code_mode_host::GrpcCodeModeHost::new(),
            ),
        );
        axum::serve(listener, app).await
    }));
    let provider = GrpcCodeModeSessionProvider::new(format!("http://{address}"));
    let session = timeout(TEST_TIMEOUT, provider.create_session())
        .await?
        .map_err(anyhow::Error::msg)?;
    let response = execute(
        &session,
        request("text(42);"),
        Arc::new(NoopCodeModeSessionDelegate),
    )
    .await?;
    assert_eq!(
        response,
        text_response("1", "42", response.code_mode_host_duration())
    );
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    drop(server);
    Ok(())
}
