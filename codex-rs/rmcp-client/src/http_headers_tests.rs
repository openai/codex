use super::*;
use pretty_assertions::assert_eq;

#[test]
fn helper_output_errors_do_not_echo_secrets() {
    for output in [
        br#"{"Host":"secret"}"#.as_slice(),
        br#"{"secret":"secret","secret":"secret"}"#.as_slice(),
        br#"{"secret":"secret","Secret":"secret"}"#.as_slice(),
    ] {
        let error = parse_helper_output(output.to_vec()).expect_err("invalid helper output");
        assert!(!error.to_string().contains("secret"));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn helper_attempt_is_shared_after_cancellation() {
    use tempfile::tempdir;

    let temp = tempdir().expect("temporary helper directory");
    let cwd = temp
        .path()
        .canonicalize()
        .expect("canonical helper directory");
    let cancelled_invocations = cwd.join("cancelled-invocations");
    let cancelled_finished = cwd.join("cancelled-helper-finished");
    let cancelled = HttpHeadersProvider::new(
        "https://example.com",
        &format!(
            "test \"$(pwd)\" = '{0}'; test -n \"$HOME\"; test -n \"$PATH\"; \
             printf x >> '{1}'; sleep 0.2; printf x > '{2}'; \
             printf '{{\"X-Gateway\":\"token\"}}'",
            cwd.display(),
            cancelled_invocations.display(),
            cancelled_finished.display(),
        ),
        cwd.clone(),
    )
    .expect("cancelled provider");
    assert!(
        tokio::time::timeout(Duration::from_millis(20), cancelled.headers())
            .await
            .is_err()
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(cancelled_finished.exists());
    assert!(cancelled.headers().await.is_ok());
    assert_eq!(
        std::fs::read_to_string(&cancelled_invocations).expect("cancelled invocation count"),
        "x"
    );
    std::fs::remove_file(&cancelled_finished).expect("reset helper completion marker");
    let headers = cancelled.headers().await.expect("cached headers");
    assert!(
        tokio::time::timeout(
            Duration::from_millis(/*millis*/ 20),
            cancelled.refresh(headers.refresh_epoch)
        )
        .await
        .is_err()
    );
    tokio::time::sleep(Duration::from_millis(/*millis*/ 500)).await;
    assert!(cancelled_finished.exists());
    assert_eq!(
        cancelled.refresh(headers.refresh_epoch).await.unwrap(),
        headers.values
    );
    assert_eq!(
        std::fs::read_to_string(cancelled_invocations).expect("refresh invocation count"),
        "xx"
    );

    let dropped_started = cwd.join("dropped-helper-started");
    let dropped_finished = cwd.join("dropped-helper-finished");
    let dropped = HttpHeadersProvider::new(
        "https://example.com",
        &format!(
            "sleep 0.6; printf '%s\\n' \"$$\" > '{}'; sleep 30; printf x > '{}'",
            dropped_started.display(),
            dropped_finished.display(),
        ),
        cwd,
    )
    .expect("dropped provider");
    // A slow spawn must not race cancellation. Wait for the helper's PID before
    // dropping the caller future, leaving the provider as the attempt's owner.
    let dropped_pid = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            _ = dropped.headers() => panic!("helper completed before cancellation"),
            pid = async {
                loop {
                    match std::fs::read_to_string(&dropped_started) {
                        Ok(pid) if pid.ends_with('\n') => {
                            break pid.trim().parse::<libc::pid_t>().expect("helper PID");
                        }
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => panic!("failed to read helper PID: {error}"),
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            } => pid,
        }
    })
    .await
    .expect("helper started before cancellation");
    assert!(dropped_pid > 0);
    drop(dropped);
    // Observe cleanup before HELPER_TIMEOUT or the helper's sleep could finish.
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            // SAFETY: signal 0 only checks whether the known child PID exists.
            let result = unsafe {
                libc::kill(dropped_pid, /* sig */ 0)
            };
            if result == -1 {
                assert_eq!(
                    std::io::Error::last_os_error().raw_os_error(),
                    Some(libc::ESRCH)
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("dropping the provider terminated the helper");
    assert!(!dropped_finished.exists());
}

#[tokio::test]
async fn nonzero_helper_exit_is_cached() {
    let temp = tempfile::tempdir().expect("temporary helper directory");
    let failed_invocations = temp.path().join("failed-invocations");
    let command = if cfg!(windows) {
        format!(
            r#"echo x>>"{0}" & echo {{"X-Gateway":"valid"}} & exit /b 23"#,
            failed_invocations.display()
        )
    } else {
        format!(
            "echo x >> '{0}'; printf '{{\"X-Gateway\":\"valid\"}}'; exit 23",
            failed_invocations.display()
        )
    };
    let failed = HttpHeadersProvider::new(
        "https://example.com/mcp",
        &command,
        temp.path().to_path_buf(),
    )
    .expect("failed provider");
    let first = failed.headers().await.err().expect("failed helper");
    let second = failed.headers().await.err().expect("cached failure");
    assert_eq!(first.to_string(), second.to_string());
    let invocations = std::fs::read_to_string(failed_invocations).expect("failed invocation count");
    assert_eq!(invocations.lines().count(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn failed_refresh_is_shared_without_replacing_credentials() {
    let temp = tempfile::tempdir().expect("temporary helper directory");
    let invocations = temp.path().join("invocations");
    let command = format!(
        "printf x >> '{0}'; count=$(wc -c < '{0}'); \
         if [ \"$count\" -gt 1 ]; then exit 23; fi; \
         printf '{{\"X-Gateway\":\"token\"}}'",
        invocations.display(),
    );
    let provider = HttpHeadersProvider::new(
        "https://example.com/mcp",
        &command,
        temp.path().to_path_buf(),
    )
    .expect("headers provider");
    let original = provider.headers().await.expect("initial helper headers");
    assert!(provider.refresh(original.refresh_epoch).await.is_err());
    let stale = provider.refresh(original.refresh_epoch).await.unwrap();
    assert_eq!(stale, original.values);
    let next = provider.headers().await.expect("preserved credentials");
    assert_eq!(next.values, original.values);
    assert!(provider.refresh(next.refresh_epoch).await.is_err());
    assert_eq!(std::fs::read_to_string(invocations).unwrap(), "xxx");
}

#[cfg(unix)]
#[tokio::test]
async fn connection_headers_are_cached_and_origin_bound() {
    use axum::Router;
    use axum::http::StatusCode;
    use axum::response::Redirect;
    use axum::routing::get;
    use axum::routing::post;
    use codex_exec_server::RouteAwareHttpClient;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::net::TcpListener;

    async fn handle(headers: axum::http::HeaderMap) -> StatusCode {
        assert_eq!(
            headers.get("proxy-authorization"),
            Some(&HeaderValue::from_static("Bearer token"))
        );
        assert_eq!(
            headers.get("x-label"),
            Some(&HeaderValue::from_bytes("café".as_bytes()).unwrap())
        );
        StatusCode::NO_CONTENT
    }
    let temp = tempdir().expect("temporary helper directory");
    let invocation_file = temp.path().join("invocations");
    let cross_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind cross-origin server");
    let cross_url = format!("http://{}/start", cross_listener.local_addr().unwrap());
    tokio::spawn(async move {
        axum::serve(
            cross_listener,
            Router::new()
                .route("/start", get(|| async { Redirect::temporary("/final") }))
                .route("/final", get(|| async { StatusCode::NO_CONTENT })),
        )
        .await
        .expect("serve cross-origin requests");
    });
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind test server");
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    let redirect_url = cross_url.clone();
    let app = Router::new().route("/mcp", post(handle)).route(
        "/redirect",
        get(move || std::future::ready(Redirect::temporary(&redirect_url))),
    );
    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("serve test requests");
    });
    let command = format!(
        "printf x >> '{}'; printf '{{\"Proxy-Authorization\":\"Bearer token\",\"X-Label\":\"café\"}}'",
        invocation_file.display(),
    );
    let inner: Arc<dyn HttpClient> = Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    )));
    let client = with_http_headers_helper(inner, &url, &command, temp.path().to_path_buf())
        .expect("headers helper client");
    let request = |session: &str| HttpRequestParams {
        method: "POST".to_string(),
        url: url.clone(),
        headers: Vec::new(),
        body: None,
        timeout_ms: Some(5_000),
        redirect_policy: HttpRedirectPolicy::Follow,
        request_id: session.to_string(),
        stream_response: true,
    };
    let mut cross_request = request("cross-origin");
    cross_request.method = "GET".to_string();
    cross_request.url = cross_url;
    assert_eq!(
        client.http_request(cross_request).await.unwrap().status,
        204
    );
    assert!(!invocation_file.exists());
    let mut redirect_request = request("redirect");
    redirect_request.method = "GET".to_string();
    redirect_request.url = url.replace("/mcp", "/redirect");
    assert_eq!(
        client.http_request(redirect_request).await.unwrap().status,
        307
    );
    let (left, right) = tokio::join!(
        client.http_request_stream(request("session-a")),
        client.http_request_stream(request("session-b"))
    );
    assert_eq!(left.expect("left request").0.status, 204);
    assert_eq!(right.expect("right request").0.status, 204);
    assert_eq!(
        std::fs::read_to_string(&invocation_file).expect("helper invocation count"),
        "x"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn concurrent_rejected_posts_share_one_headers_refresh() {
    use axum::Router;
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::routing::post;
    use codex_exec_server::RouteAwareHttpClient;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use tempfile::tempdir;
    use tokio::net::TcpListener;
    use tokio::sync::Barrier;

    for (rejection, credential_header) in [
        (StatusCode::UNAUTHORIZED, "Proxy-Authorization"),
        (StatusCode::FORBIDDEN, "x-litellm-api-key"),
        (StatusCode::UNAUTHORIZED, "Authorization"),
    ] {
        let temp = tempdir().expect("temporary helper directory");
        let invocations = temp.path().join("invocations");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let barrier = Arc::new(Barrier::new(/*n*/ 2));
        let app =
            Router::new()
                .route(
                    "/mcp",
                    post(
                        move |State(barrier): State<Arc<Barrier>>,
                              headers: HeaderMap,
                              body: String| async move {
                            assert_eq!(body, "original request body");
                            assert_eq!(
                                headers.get("mcp-session-id"),
                                Some(&HeaderValue::from_static("existing-session"))
                            );
                            match headers[credential_header].to_str().unwrap() {
                                "Bearer token-1" => {
                                    barrier.wait().await;
                                    rejection
                                }
                                "Bearer token-2" => {
                                    assert_eq!(headers["x-helper-only"], "configured");
                                    StatusCode::NO_CONTENT
                                }
                                unexpected => panic!("unexpected test credential: {unexpected}"),
                            }
                        },
                    ),
                )
                .with_state(barrier);
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let command = format!(
            "printf x >> '{0}'; count=$(wc -c < '{0}'); count=$((count)); \
             if [ \"$count\" -eq 1 ]; then \
                 printf '{{\"{credential_header}\":\"Bearer token-1\",\"X-Helper-Only\":\"stale\"}}'; \
             else printf '{{\"{credential_header}\":\"Bearer token-%s\"}}' \"$count\"; fi",
            invocations.display(),
        );
        let inner: Arc<dyn HttpClient> = Arc::new(RouteAwareHttpClient::new(
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ));
        let client = with_http_headers_helper(inner, &url, &command, temp.path().to_path_buf())
            .expect("headers helper client");
        let request = |request_id: &str| HttpRequestParams {
            method: "POST".to_string(),
            url: url.clone(),
            headers: vec![
                HttpHeader {
                    name: "X-Helper-Only".to_string(),
                    value: "configured".to_string(),
                    value_env_var: None,
                },
                HttpHeader {
                    name: "mcp-session-id".to_string(),
                    value: "existing-session".to_string(),
                    value_env_var: None,
                },
            ],
            body: Some(b"original request body".to_vec().into()),
            timeout_ms: Some(5_000),
            redirect_policy: HttpRedirectPolicy::Follow,
            request_id: request_id.to_string(),
            stream_response: true,
        };

        let (left, right) = tokio::join!(
            client.http_request(request("left")),
            client.http_request_stream(request("right")),
        );
        assert_eq!(left.expect("left request").status, 204);
        assert_eq!(right.expect("right request").0.status, 204);
        assert_eq!(
            std::fs::read_to_string(invocations).expect("helper invocation count"),
            "xx"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn helper_refresh_preserves_oauth_challenges_and_retries_at_most_once() {
    use codex_exec_server::RouteAwareHttpClient;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use tempfile::tempdir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;

    let unchanged = "printf '{\"Proxy-Authorization\":\"Bearer token\"}'";
    let changed = "printf '{\"Proxy-Authorization\":\"Bearer token-%s\"}' \"$count\"";
    let failed = "if [ \"$count\" -gt 1 ]; then exit 23; fi; printf '{\"Proxy-Authorization\":\"Bearer token\"}'";
    let ignored_authorization = "if [ \"$count\" -eq 1 ]; then \
        printf '{\"Authorization\":\"old\",\"X-A\":\"a\",\"X-B\":\"b\"}'; \
        else printf '{\"X-B\":\"b\",\"X-A\":\"a\",\"Authorization\":\"new\"}'; fi";
    for streamed in [false, true] {
        for (helper_output, status, expected_mcp_requests) in [
            (unchanged, 401, 1),
            (failed, 401, 1),
            (changed, 401, 2),
            (changed, 403, 1),
            (ignored_authorization, 401, 1),
        ] {
            let (challenge, expected_helper_invocations) = if status == 403 {
                (
                    r#"Bearer error="insufficient_scope", scope="tools:write""#,
                    "x",
                )
            } else {
                (r#"Bearer realm="oauth""#, "xx")
            };
            let temp = tempdir().expect("temporary helper directory");
            let invocations = temp.path().join("invocations");
            let server = MockServer::start().await;
            let url = format!("{}/mcp", server.uri());
            Mock::given(method("POST"))
                .and(header("authorization", "Bearer oauth-token"))
                .respond_with(
                    ResponseTemplate::new(status)
                        .insert_header("www-authenticate", challenge)
                        .set_body_string("original OAuth challenge"),
                )
                .expect(expected_mcp_requests)
                .mount(&server)
                .await;
            let command = format!(
                "printf x >> '{0}'; count=$(wc -c < '{0}'); {helper_output}",
                invocations.display(),
            );
            let inner: Arc<dyn HttpClient> = Arc::new(RouteAwareHttpClient::new(
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            ));
            let client = with_http_headers_helper(inner, &url, &command, temp.path().to_path_buf())
                .expect("headers helper client");
            let params = HttpRequestParams {
                method: "POST".to_string(),
                url: url.clone(),
                headers: vec![HttpHeader {
                    name: "aUtHoRiZaTiOn".to_string(),
                    value: "Bearer oauth-token".to_string(),
                    value_env_var: None,
                }],
                body: None,
                timeout_ms: Some(5_000),
                redirect_policy: HttpRedirectPolicy::Follow,
                request_id: "oauth-challenge".to_string(),
                stream_response: streamed,
            };
            let (response, body) = if streamed {
                let (response, mut body) = client
                    .http_request_stream(params)
                    .await
                    .expect("original OAuth response");
                let mut bytes = Vec::new();
                while let Some(chunk) = body.recv().await.expect("response body chunk") {
                    bytes.extend(chunk);
                }
                (response, bytes)
            } else {
                let response = client
                    .http_request(params)
                    .await
                    .expect("original OAuth response");
                let bytes = response.body.clone().into_inner();
                (response, bytes)
            };

            assert_eq!(response.status, status);
            assert!(response.headers.iter().any(|header| {
                header.name.eq_ignore_ascii_case("www-authenticate") && header.value == challenge
            }));
            assert_eq!(body, b"original OAuth challenge");
            assert_eq!(
                std::fs::read_to_string(invocations).expect("helper invocation count"),
                expected_helper_invocations
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn refresh_retry_rechecks_deadline_and_redirects() {
    use codex_exec_server::RouteAwareHttpClient;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;

    let temp = tempfile::tempdir().expect("helper directory");
    let command = "if [ -e invoked ]; then printf '{\"Proxy-Authorization\":\"Bearer fresh\"}'; else touch invoked; printf '{}'; fi";
    let client = HttpHeadersClient {
        inner: Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        provider: HttpHeadersProvider::new(
            "http://example.com/mcp",
            command,
            temp.path().to_path_buf(),
        )
        .expect("provider"),
    };
    let requests = Mutex::new(Vec::new());
    let error = client
        .request(
            HttpRequestParams {
                method: "POST".to_string(),
                url: "http://example.com/mcp".to_string(),
                headers: Vec::new(),
                body: Some(b"original request".to_vec().into()),
                timeout_ms: Some(5_000),
                redirect_policy: HttpRedirectPolicy::Stop,
                request_id: "mcp-request".to_string(),
                stream_response: false,
            },
            |params| {
                let mut requests = requests.lock().expect("requests lock");
                let status = if requests.is_empty() { 401 } else { 307 };
                requests.push(params);
                async move {
                    tokio::time::sleep(Duration::from_millis(/*millis*/ 20)).await;
                    Ok(HttpRequestResponse {
                        status,
                        headers: vec![HttpHeader {
                            name: "Location".to_string(),
                            value: "http://example.com/redirected".to_string(),
                            value_env_var: None,
                        }],
                        body: Vec::new().into(),
                    })
                }
                .boxed()
            },
            |response| response,
        )
        .await
        .expect_err("refreshed proxy redirect must fail");
    assert!(
        error
            .to_string()
            .contains("cannot safely replay Proxy-Authorization")
    );
    let requests = requests.into_inner().expect("recorded requests");
    let [original, retry] = requests.as_slice() else {
        panic!("expected exactly two requests");
    };
    assert!(retry.timeout_ms < original.timeout_ms);
    let mut expected_retry = original.clone();
    expected_retry.timeout_ms = retry.timeout_ms;
    expected_retry.headers = vec![HttpHeader {
        name: "proxy-authorization".to_string(),
        value: "Bearer fresh".to_string(),
        value_env_var: None,
    }];
    assert_eq!(retry, &expected_retry);
}
