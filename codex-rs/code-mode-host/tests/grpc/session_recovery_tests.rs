//! Exercises missing-session recovery while the host's lease streams remain open.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Result;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::GrpcCodeModeSessionProvider;
use codex_code_mode::NoopCodeModeSessionDelegate;
use codex_code_mode_host::GrpcCodeModeHost;
use codex_code_mode_protocol::grpc::code_mode_host_server::CodeModeHostServer;
use pretty_assertions::assert_eq;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::task::AbortOnDropHandle;
use tonic::Code;
use tonic::Status;
use tonic::transport::Server;

use super::execute;
use super::request;
use super::text_response;

#[tokio::test]
async fn only_missing_sessions_reopen_without_replaying_execution() -> Result<()> {
    for (code, expected_cell, expected_value) in [
        (Code::NotFound, "g2:1", "undefined"),
        (Code::InvalidArgument, "2", "1"),
    ] {
        let rejection = Arc::new(Mutex::new(/*t*/ None));
        let next_rejection = Arc::clone(&rejection);
        let request_count = Arc::new(AtomicUsize::new(/*v*/ 0));
        let count = Arc::clone(&request_count);
        let service = CodeModeHostServer::with_interceptor(
            GrpcCodeModeHost::new(),
            move |request: tonic::Request<()>| {
                count.fetch_add(/*val*/ 1, Ordering::SeqCst);
                match next_rejection.lock().unwrap().take() {
                    Some(rejection) => Err(rejection),
                    None => Ok(request),
                }
            },
        );
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let _server = AbortOnDropHandle::new(tokio::spawn(
            Server::builder()
                .add_service(service)
                .serve_with_incoming(TcpListenerStream::new(listener)),
        ));
        let session = GrpcCodeModeSessionProvider::new(endpoint)
            .create_session()
            .await
            .map_err(anyhow::Error::msg)?;
        execute(
            &session,
            request(r#"store("before", 1);"#),
            Arc::new(NoopCodeModeSessionDelegate),
        )
        .await?;

        // Reject the next Execute without closing the lease or subscription streams.
        let failure = Status::new(code, "injected RPC rejection");
        *rejection.lock().unwrap() = Some(failure.clone());
        let requests_before_rejection = request_count.load(Ordering::SeqCst);
        let failed = execute(
            &session,
            request(r#"store("before", "replayed");"#),
            Arc::new(NoopCodeModeSessionDelegate),
        )
        .await
        .unwrap_err();
        assert_eq!(
            failed.to_string(),
            format!("gRPC code-mode execution failed: {failure}")
        );
        assert_eq!(
            request_count.load(Ordering::SeqCst),
            requests_before_rejection + 1,
            "the failed call must return without opening a session or replaying Execute"
        );
        let actual = execute(
            &session,
            request(r#"text(String(load("before")))"#),
            Arc::new(NoopCodeModeSessionDelegate),
        )
        .await?;
        assert_eq!(
            actual,
            text_response(
                expected_cell,
                expected_value,
                actual.code_mode_host_duration()
            )
        );
        session.shutdown().await.map_err(anyhow::Error::msg)?;
    }
    Ok(())
}
