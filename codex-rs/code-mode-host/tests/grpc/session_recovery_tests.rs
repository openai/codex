//! Exercises missing-session recovery while the host's lease streams remain open.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Result;
use codex_code_mode::CodeModeOutputTruncation;
use codex_code_mode::CodeModeSessionOptions;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::FunctionCallOutputContentItem;
use codex_code_mode::GrpcCodeModeSessionProvider;
use codex_code_mode::NoopCodeModeSessionDelegate;
use codex_code_mode::RuntimeResponse;
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
    for (code, route_lost, expected_cell, expected_value) in [
        (Code::NotFound, false, "g2:1", "undefined"),
        (Code::InvalidArgument, false, "2", "1"),
        (Code::Unavailable, false, "2", "1"),
        (Code::Unavailable, true, "2", "1"),
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
        assert_eq!(
            session.output_truncation(),
            CodeModeOutputTruncation::Client
        );
        execute(
            &session,
            request(r#"store("before", 1);"#),
            Arc::new(NoopCodeModeSessionDelegate),
        )
        .await?;

        // Reject the next Execute without closing the lease or subscription streams.
        let mut failure = Status::new(code, "injected RPC rejection");
        if route_lost {
            // A direct host has no proxy route to retire.
            failure
                .metadata_mut()
                .insert("x-code-mode-route-lost", "true".parse()?);
        }
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
        if code == Code::NotFound {
            let reset = execute(
                &session,
                request(r#"store("before", "must not run");"#),
                Arc::new(NoopCodeModeSessionDelegate),
            )
            .await
            .unwrap_err();
            assert!(
                reset
                    .to_string()
                    .contains("Stored values and running cells were lost")
            );
        }
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

#[tokio::test]
async fn grpc_session_pins_output_mode_across_recovery_and_shutdown() -> Result<()> {
    use CodeModeOutputTruncation::Client;
    use CodeModeOutputTruncation::Server;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::middleware::Next;

    // A Server-capable host honors either choice. A Client-only host ignores the new field.
    for (initial_host, replacement_host) in [(Server, Server), (Server, Client), (Client, Server)] {
        let opens = Arc::new(AtomicUsize::new(0));
        let subscriptions = Arc::new(AtomicUsize::new(0));
        let subscription_count = Arc::clone(&subscriptions);
        let app = axum::Router::new()
            .fallback_service(CodeModeHostServer::new(GrpcCodeModeHost::new()))
            .layer(axum::middleware::from_fn(
                move |mut rpc: Request, next: Next| {
                    let opens = Arc::clone(&opens);
                    let subscriptions = Arc::clone(&subscription_count);
                    async move {
                        if rpc.uri().path().ends_with("/OpenSession") {
                            let host = if opens.fetch_add(1, Ordering::SeqCst) == 0 {
                                initial_host
                            } else {
                                replacement_host
                            };
                            if host == Client {
                                // No resource limits were requested, so ignoring the output mode
                                // leaves an empty OpenSessionRequest in one uncompressed gRPC frame.
                                rpc.headers_mut().remove("content-length");
                                *rpc.body_mut() = Body::from(&[0_u8; 5][..]);
                            }
                        } else if rpc.uri().path().ends_with("/Execute")
                            && opens.load(Ordering::SeqCst) == 1
                        {
                            return Status::not_found("session was lost").into_http::<Body>();
                        } else if rpc.uri().path().ends_with("/SubscribeToToolCalls") {
                            subscriptions.fetch_add(1, Ordering::SeqCst);
                        }
                        next.run(rpc).await
                    }
                },
            ));
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let _server = AbortOnDropHandle::new(tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("recovery test host");
        }));
        let session = GrpcCodeModeSessionProvider::new(endpoint)
            .create_session_with_options(CodeModeSessionOptions {
                output_truncation: Server,
                ..CodeModeSessionOptions::default()
            })
            .await
            .map_err(anyhow::Error::msg)?;
        assert_eq!(session.output_truncation(), initial_host);

        let delegate = Arc::new(NoopCodeModeSessionDelegate);
        let failed = execute(&session, request("text('must not run')"), delegate.clone())
            .await
            .unwrap_err();
        assert!(failed.to_string().contains("session was lost"));
        let reset = execute(
            &session,
            request("text('must not replay')"),
            delegate.clone(),
        )
        .await
        .unwrap_err();
        if initial_host == Server && replacement_host == Client {
            assert_eq!(
                reset.to_string(),
                "gRPC code-mode host changed the session output truncation mode on reconnect"
            );
            assert_eq!(
                subscriptions.load(Ordering::SeqCst),
                1,
                "the downgraded binding must not subscribe"
            );
        } else {
            assert!(
                reset
                    .to_string()
                    .contains("Stored values and running cells were lost")
            );
            let source = match initial_host {
                Client => "text('x'.repeat(2 * 1024 * 1024))",
                Server => "throw 'begin:' + 'X'.repeat(65 * 1024 * 1024) + ':end'",
            };
            let mut observation = request(source);
            observation.max_output_tokens = Some(5);
            observation.yield_time_ms = Some(60_000);
            let response = execute(&session, observation, delegate).await?;
            match initial_host {
                Client => assert_eq!(
                    response,
                    text_response(
                        "g2:1",
                        &"x".repeat(2 * 1024 * 1024),
                        response.code_mode_host_duration()
                    )
                ),
                Server => {
                    let RuntimeResponse::Result {
                        cell_id,
                        content_items,
                        error_text: Some(diagnostic),
                        ..
                    } = response
                    else {
                        panic!("expected a script failure");
                    };
                    assert_eq!(cell_id.as_str(), "g2:1");
                    assert_eq!(diagnostic.len(), 64 * 1024);
                    assert!(diagnostic.starts_with("begin:") && diagnostic.ends_with(":end"));
                    assert_eq!(content_items, [
                        "Warning: truncated output (original token count: 262144)\nTotal output lines: 2\n\nScript err…262139 tokens truncated…",
                        "[Code Mode session output limit reached: omitted 67108888 text bytes; discarded 0 text segments, 0 images, 0 audio items]",
                        "XXXXXX:end",
                    ].map(|text| FunctionCallOutputContentItem::InputText { text: text.into() }));
                }
            }
        }
        session.shutdown().await.map_err(anyhow::Error::msg)?;
        assert_eq!(session.output_truncation(), initial_host);
    }
    Ok(())
}
