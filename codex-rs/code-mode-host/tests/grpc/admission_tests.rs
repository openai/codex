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
        (Code::Unavailable, usize::MAX, 5, false),
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

#[tokio::test]
async fn routing_metadata_is_scoped_to_each_session_on_a_shared_provider() -> Result<()> {
    use axum::body::Body;
    use axum::extract::Request;
    use axum::response::Response;
    use tonic::codegen::Service;

    let backends = Arc::new(["a", "b"].map(|_| {
        axum::Router::new().fallback_service(grpc::code_mode_host_server::CodeModeHostServer::new(
            codex_code_mode_host::GrpcCodeModeHost::new(),
        ))
    }));
    let admissions = Arc::new(AtomicUsize::new(0));
    let rejection = Arc::new(std::sync::Mutex::new(/*t*/ None::<tonic::Status>));
    let rejected_executions = Arc::new(AtomicUsize::new(0));
    let next_rejection = Arc::clone(&rejection);
    let executions = Arc::clone(&rejected_executions);
    let opens = Arc::clone(&admissions);
    let app = axum::Router::new().fallback(move |request: Request| {
        let backends = Arc::clone(&backends);
        let admissions = Arc::clone(&admissions);
        let rejection = Arc::clone(&next_rejection);
        let executions = Arc::clone(&executions);
        async move {
            let opening = request.uri().path().ends_with("/OpenSession");
            let route = request.headers().get("x-code-mode-route");
            let token = if opening {
                assert_eq!(
                    route, None,
                    "a new admission must not inherit any session's route"
                );
                match admissions.fetch_add(1, Ordering::SeqCst) {
                    0 => {
                        // Even a rejected admission can carry response metadata. The
                        // next attempt must still be free to select another backend.
                        let mut response =
                            tonic::Status::resource_exhausted("full").into_http::<Body>();
                        response
                            .headers_mut()
                            .insert("x-code-mode-route", "rejected".parse().unwrap());
                        return response;
                    }
                    1 => "a",
                    _ => "b",
                }
            } else {
                route
                    .expect("every session RPC needs its routing token")
                    .to_str()
                    .unwrap()
            };
            let index = match token {
                "a" => 0,
                "b" => 1,
                _ => panic!("unexpected routing token"),
            };
            if token == "a"
                && request.uri().path().ends_with("/Execute")
                && let Some(error) = rejection.lock().unwrap().as_ref()
            {
                executions.fetch_add(1, Ordering::SeqCst);
                return error.clone().into_http::<Body>();
            }
            let mut response: Response = backends[index].clone().call(request).await.unwrap();
            if opening {
                response
                    .headers_mut()
                    .insert("x-code-mode-route", ["a", "b"][index].parse().unwrap());
            }
            response
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let _server = tokio_util::task::AbortOnDropHandle::new(tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("routing test server");
    }));
    let provider = GrpcCodeModeSessionProvider::new(format!("http://{address}"));
    let parent = provider
        .create_session()
        .await
        .map_err(anyhow::Error::msg)?;
    let child = provider
        .create_session()
        .await
        .map_err(anyhow::Error::msg)?;
    for (session, value) in [(&parent, 11), (&child, 22), (&parent, 33)] {
        let mut req = request(&format!(
            "await tools.echo({{value: {value}}}); text({value});"
        ));
        req.enabled_tools = vec![tool("echo")];
        let actual = execute(session, req, Arc::new(RecordingDelegate::default())).await?;
        let RuntimeResponse::Result { error_text, .. } = actual else {
            anyhow::bail!("session did not finish");
        };
        assert_eq!(error_text, None);
        let mut req = request("await new Promise(() => {});");
        req.yield_time_ms = Some(1);
        let started = session
            .execute(
                req,
                Arc::new(NoopCodeModeSessionDelegate),
                /*preempt*/ None,
            )
            .await
            .map_err(anyhow::Error::msg)?;
        let id = started.cell_id.clone();
        assert!(matches!(
            started
                .initial_response()
                .await
                .map_err(anyhow::Error::msg)?,
            RuntimeResponse::Yielded { .. }
        ));
        let outcome = session
            .wait(
                WaitRequest {
                    cell_id: id.clone(),
                    yield_time_ms: 1,
                },
                /*preempt*/ None,
            )
            .await
            .map_err(anyhow::Error::msg)?;
        assert!(matches!(
            outcome,
            WaitOutcome::LiveCell(RuntimeResponse::Yielded { .. })
        ));
        session.terminate(id).await.map_err(anyhow::Error::msg)?;
    }
    for session in [&parent, &child] {
        execute(
            session,
            request(r#"store("preserved", "original");"#),
            Arc::new(NoopCodeModeSessionDelegate),
        )
        .await?;
    }
    for retired in [false, true] {
        let mut error = tonic::Status::unavailable("no healthy upstream");
        if retired {
            error
                .metadata_mut()
                .insert("x-code-mode-route-lost", "true".parse()?);
        }
        *rejection.lock().unwrap() = Some(error);
        let opens_before = opens.load(Ordering::SeqCst);
        let failures_before = rejected_executions.load(Ordering::SeqCst);
        execute(
            &parent,
            request(r#"store("preserved", "replayed");"#),
            Arc::new(NoopCodeModeSessionDelegate),
        )
        .await
        .expect_err("the rejected execution must not be replayed");
        assert_eq!(opens.load(Ordering::SeqCst), opens_before);
        assert_eq!(
            rejected_executions.load(Ordering::SeqCst),
            failures_before + 1
        );
        if !retired {
            *rejection.lock().unwrap() = None;
        }
        // The old lease and callback streams are still open. Only the explicit
        // route-loss response may replace the parent; the child must keep its state.
        for (session, expected) in [
            (&parent, if retired { "undefined" } else { "original" }),
            (&child, "original"),
        ] {
            let actual = execute(
                session,
                request(r#"text(String(load("preserved")));"#),
                Arc::new(NoopCodeModeSessionDelegate),
            )
            .await?;
            let RuntimeResponse::Result { cell_id, .. } = &actual else {
                anyhow::bail!("execution did not finish");
            };
            assert_eq!(
                actual,
                text_response(cell_id.as_str(), expected, actual.code_mode_host_duration())
            );
        }
        assert_eq!(
            opens.load(Ordering::SeqCst),
            opens_before + usize::from(retired)
        );
    }
    parent.shutdown().await.map_err(anyhow::Error::msg)?;
    child.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}
