use std::io::ErrorKind;

use codex_exec_server::HttpRedirectPolicy;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use pretty_assertions::assert_eq;

use super::HttpHeader;
use super::SseEventSizeLimit;
use super::StreamableHttpRedirectMode;
use super::mcp_redirect_policy;
use super::protocol_headers;

#[test]
fn protocol_headers_preserve_utf8_values() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-plugin-name",
        HeaderValue::from_str("café").expect("valid HTTP field value"),
    );

    assert_eq!(
        protocol_headers(&headers),
        vec![HttpHeader {
            name: "x-plugin-name".to_string(),
            value: "café".to_string(),
            value_env_var: None,
        }]
    );
}

#[test]
fn legacy_configured_headers_follow_redirects() {
    assert_eq!(
        mcp_redirect_policy(
            StreamableHttpRedirectMode::Legacy,
            &HeaderMap::new(),
            /*has_configured_headers*/ true,
        ),
        HttpRedirectPolicy::Follow
    );
}

#[test]
fn agent_plugin_configured_headers_stop_redirects() {
    assert_eq!(
        mcp_redirect_policy(
            StreamableHttpRedirectMode::AgentPluginV1,
            &HeaderMap::new(),
            /*has_configured_headers*/ true,
        ),
        HttpRedirectPolicy::Stop
    );
}

#[test]
fn requests_without_sensitive_headers_follow_redirects() {
    assert_eq!(
        mcp_redirect_policy(
            StreamableHttpRedirectMode::AgentPluginV1,
            &HeaderMap::new(),
            /*has_configured_headers*/ false,
        ),
        HttpRedirectPolicy::Follow
    );
}

#[test]
fn authorization_redirects_depend_on_mode() {
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
    assert_eq!(
        mcp_redirect_policy(
            StreamableHttpRedirectMode::Legacy,
            &headers,
            /*has_configured_headers*/ false,
        ),
        HttpRedirectPolicy::Follow
    );
    assert_eq!(
        mcp_redirect_policy(
            StreamableHttpRedirectMode::AgentPluginV1,
            &headers,
            /*has_configured_headers*/ false,
        ),
        HttpRedirectPolicy::Stop
    );
}

#[test]
fn runtime_assertions_stop_redirects_in_both_mcp_modes() {
    for mode in [
        StreamableHttpRedirectMode::Legacy,
        StreamableHttpRedirectMode::AgentPluginV1,
    ] {
        for (name, value) in [
            ("x-openai-agent-assertion", "1.protected.payload.signature"),
            (
                "authorization",
                "AgentAssertion 1.protected.payload.signature",
            ),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(name, HeaderValue::from_static(value));
            assert_eq!(
                mcp_redirect_policy(mode, &headers, /*has_configured_headers*/ false),
                HttpRedirectPolicy::Stop,
            );
        }
    }
}

#[test]
fn lf_terminators_reset_the_event_limit() {
    let mut limit = SseEventSizeLimit::new(Some(8));

    limit
        .observe(b"data: a\n\ndata: b\n\n")
        .expect("LF-terminated events must have independent size limits");

    assert_eq!((limit.retained_bytes, limit.line_bytes), (0, 0));
}

#[test]
fn carriage_return_terminators_reset_the_event_limit() {
    let mut limit = SseEventSizeLimit::new(Some(8));

    limit
        .observe(b"data: a\r\rdata: b\r\r")
        .expect("CR-terminated events must have independent size limits");

    assert_eq!((limit.retained_bytes, limit.line_bytes), (0, 0));
}

#[test]
fn crlf_terminators_split_across_chunks_reset_the_event_limit() {
    let mut limit = SseEventSizeLimit::new(Some(8));

    limit
        .observe(b"data: a\r")
        .expect("a CR must finish the first event field");
    limit
        .observe(b"\n\r")
        .expect("a split CRLF must not finish another field");
    limit
        .observe(b"\ndata: b\r")
        .expect("the blank CRLF must reset the first event");
    limit
        .observe(b"\n\r\n")
        .expect("the second split CRLF event must remain within the limit");

    assert_eq!((limit.retained_bytes, limit.line_bytes), (0, 0));
}

#[test]
fn event_at_the_exact_size_limit_is_accepted() {
    let mut limit = SseEventSizeLimit::new(Some(9));

    limit
        .observe(b"data: ab\n\n")
        .expect("an event at the exact limit must be accepted");

    assert_eq!((limit.retained_bytes, limit.line_bytes), (0, 0));
}

#[test]
fn completed_keepalive_comments_do_not_accumulate() {
    let mut limit = SseEventSizeLimit::new(Some(6));

    limit
        .observe(&b": ping\n".repeat(/*n*/ 64))
        .expect("completed keepalive comments must not accumulate");

    assert_eq!((limit.retained_bytes, limit.line_bytes), (0, 0));
}

#[test]
fn split_keepalive_comments_are_discarded_only_when_complete() {
    let mut limit = SseEventSizeLimit::new(Some(6));

    limit
        .observe(b": pi")
        .expect("an incomplete comment within the limit must be retained");
    assert_eq!((limit.retained_bytes, limit.line_bytes), (0, 4));

    limit
        .observe(b"ng\n: ping\n")
        .expect("only completed comments may be excluded from the limit");

    assert_eq!((limit.retained_bytes, limit.line_bytes), (0, 0));
}

#[test]
fn comments_do_not_reset_accumulated_event_data() {
    let mut limit = SseEventSizeLimit::new(Some(14));

    limit
        .observe(b"data: a\n")
        .expect("the first data field must fit");
    limit
        .observe(b": ping\n")
        .expect("a completed comment must not count as event data");

    let error = limit
        .observe(b"data: b\n")
        .expect_err("a comment must not reset previously retained data");

    assert_eq!(
        (error.kind(), error.to_string()),
        (
            ErrorKind::InvalidData,
            "MCP response body exceeds 14 bytes".to_string(),
        )
    );
}

#[test]
fn an_unterminated_comment_remains_size_limited() {
    let mut limit = SseEventSizeLimit::new(Some(6));

    limit
        .observe(b": ping")
        .expect("a comment at the exact limit must be accepted");

    let error = limit
        .observe(b"!")
        .expect_err("an unterminated comment must not bypass the limit");

    assert_eq!(
        (error.kind(), error.to_string()),
        (
            ErrorKind::InvalidData,
            "MCP response body exceeds 6 bytes".to_string(),
        )
    );
}

#[test]
fn multiline_data_counts_parser_inserted_newlines() {
    let mut limit = SseEventSizeLimit::new(Some(18));

    let error = limit
        .observe(b"data: aaa\ndata: bbb\n\n")
        .expect_err("joined multiline event data must remain size limited");

    assert_eq!(
        (error.kind(), error.to_string()),
        (
            ErrorKind::InvalidData,
            "MCP response body exceeds 18 bytes".to_string(),
        )
    );
}

#[test]
fn legacy_sse_streams_remain_unlimited() {
    let mut limit = SseEventSizeLimit::new(/*maximum_bytes*/ None);

    limit
        .observe(&[b'x'; 128])
        .expect("legacy SSE streams must not gain a message limit");

    assert_eq!(
        (
            limit.retained_bytes,
            limit.line_bytes,
            limit.line_is_comment,
            limit.previous_was_carriage_return,
            limit.failed,
        ),
        (0, 0, false, false, false)
    );
}

#[test]
fn rejected_events_remain_rejected() {
    let mut limit = SseEventSizeLimit::new(Some(8));

    limit
        .observe(b"data: abc")
        .expect_err("an oversized event must be rejected");

    let error = limit
        .observe(b"")
        .expect_err("a rejected event must not be resumed");

    assert_eq!(
        (error.kind(), error.to_string()),
        (
            ErrorKind::InvalidData,
            "oversized MCP SSE event was already rejected".to_string(),
        )
    );
}

#[test]
fn size_accounting_saturates_without_overflow() {
    let mut limit = SseEventSizeLimit::new(Some(usize::MAX - 1));
    limit.retained_bytes = usize::MAX - 2;
    limit.line_bytes = 1;

    let error = limit
        .observe(b"x")
        .expect_err("saturating counts must still reject an oversized event");

    assert_eq!((error.kind(), limit.failed), (ErrorKind::InvalidData, true));
}

struct AsyncRequestAuth {
    requests: std::sync::atomic::AtomicUsize,
}

impl crate::AuthProviderResolver for AsyncRequestAuth {
    fn resolve<'a>(
        &'a self,
        _headers: &'a HeaderMap,
    ) -> futures::future::BoxFuture<'a, Result<codex_api::SharedAuthProvider, codex_api::AuthError>>
    {
        Box::pin(async move {
            tokio::task::yield_now().await;
            let id = self
                .requests
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(std::sync::Arc::new(SelectedRequestAuth(id)) as codex_api::SharedAuthProvider)
        })
    }
}

struct SelectedRequestAuth(usize);

impl codex_api::AuthProvider for SelectedRequestAuth {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), codex_api::AuthError> {
        headers.remove("x-openai-agent-assertion");
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("AgentAssertion proof-{}", self.0)).unwrap(),
        );
        Ok(())
    }

    fn apply_auth_headers(&self, mut headers: HeaderMap) -> codex_api::AuthHeadersFuture<'_> {
        Box::pin(async move {
            self.add_auth_headers(&mut headers)?;
            Ok(headers)
        })
    }
}

#[tokio::test]
async fn requests_await_auth_and_stop_assertion_redirects() -> anyhow::Result<()> {
    use codex_exec_server::RouteAwareHttpClient;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use futures::StreamExt;
    use rmcp::transport::streamable_http_client::StreamableHttpClient;
    use std::collections::HashMap;
    use std::sync::Arc;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(202))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).insert_header("content-type", "text/event-stream"))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/redirect"))
        .respond_with(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{}/leaked", server.uri())),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer stale"));
    headers.insert(
        "x-openai-agent-assertion",
        HeaderValue::from_static("stale-proof"),
    );
    let adapter = super::StreamableHttpClientAdapter::new(
        Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        headers,
        Some(Arc::new(AsyncRequestAuth {
            requests: Default::default(),
        })),
        /*has_configured_headers*/ false,
        StreamableHttpRedirectMode::Legacy,
        Default::default(),
    );
    let uri: Arc<str> = format!("{}/mcp", server.uri()).into();
    let notification: rmcp::model::ClientJsonRpcMessage =
        serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/initialized",
        }))?;
    for _ in 0..2 {
        adapter
            .post_message(
                uri.clone(),
                notification.clone(),
                /*session_id*/ None,
                /*auth_token*/ None,
                HashMap::new(),
            )
            .await?;
    }
    for _ in 0..2 {
        let mut stream = adapter
            .get_stream(
                uri.clone(),
                Some("session-a".into()),
                /*last_event_id*/ None,
                /*auth_token*/ None,
                HashMap::new(),
            )
            .await?;
        assert!(stream.next().await.is_none());
    }
    adapter
        .delete_session(
            uri,
            "session-a".into(),
            /*auth_token*/ None,
            HashMap::new(),
        )
        .await?;
    assert!(
        adapter
            .post_message(
                format!("{}/redirect", server.uri()).into(),
                notification,
                /*session_id*/ None,
                /*auth_token*/ None,
                HashMap::new()
            )
            .await
            .is_err()
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 6);
    for (id, request) in requests.iter().enumerate() {
        assert_eq!(
            request.headers["authorization"],
            format!("AgentAssertion proof-{id}")
        );
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
        assert_ne!(request.url.path(), "/leaked");
    }
    server.verify().await;
    Ok(())
}

#[tokio::test]
async fn automatic_event_stream_reconnect_refreshes_request_auth() -> anyhow::Result<()> {
    use codex_exec_server::RouteAwareHttpClient;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use rmcp::model::ClientCapabilities;
    use rmcp::model::Implementation;
    use rmcp::model::InitializeRequestParams;
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(|request: &wiremock::Request| {
            let body: serde_json::Value = request.body_json().unwrap();
            let result = match body["method"].as_str() {
                Some("initialize") => json!({
                    "protocolVersion": body["params"]["protocolVersion"],
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "reconnect-test", "version": "1"},
                }),
                Some("tools/call") => json!({
                    "content": [], "structuredContent": {"echo": "after reconnect"},
                }),
                _ => return ResponseTemplate::new(202),
            };
            ResponseTemplate::new(200)
                .insert_header("mcp-session-id", "session-a")
                .set_body_json(json!({"jsonrpc": "2.0", "id": body["id"], "result": result}))
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(
                concat!(
                    "retry: 1\nid: event-1\nevent: message\ndata: ",
                    r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#,
                    "\n\n",
                )
                .as_bytes(),
                "text/event-stream",
            ),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    // Leave the reconnect pending until shutdown instead of starting a tight EOF loop.
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_delay(Duration::from_secs(60)),
        )
        .with_priority(2)
        .mount(&server)
        .await;
    let client = crate::RmcpClient::new_streamable_http_client(
        "reconnect-test",
        &format!("{}/mcp", server.uri()),
        /*bearer_token*/ None,
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        Default::default(),
        Default::default(),
        Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        Some(Arc::new(AsyncRequestAuth {
            requests: Default::default(),
        })),
    )
    .await?;
    client
        .initialize(
            InitializeRequestParams::new(
                ClientCapabilities::default(),
                Implementation::new("codex-test", "1"),
            ),
            Some(Duration::from_secs(5)),
            Box::new(|_, _| Box::pin(async { anyhow::bail!("unexpected elicitation") })),
        )
        .await?;

    // The server ends the stream. Only the protocol engine, not the test, reopens it.
    let streams = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let streams: Vec<_> = server
                .received_requests()
                .await
                .unwrap()
                .into_iter()
                .filter(|request| request.method == "GET")
                .collect();
            if streams.len() >= 2 {
                break streams;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert_eq!(streams[0].headers["mcp-session-id"], "session-a");
    assert_eq!(streams[1].headers["last-event-id"], "event-1");
    assert_ne!(
        streams[0].headers["authorization"],
        streams[1].headers["authorization"]
    );
    for request in &streams {
        assert!(
            request.headers["authorization"]
                .to_str()?
                .starts_with("AgentAssertion proof-")
        );
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
    }
    let result = client
        .call_tool(
            "echo".to_string(),
            Some(json!({"message": "after reconnect"})),
            |_| Ok(None),
            Some(Duration::from_secs(5)),
        )
        .await?;
    assert_eq!(
        result.structured_content,
        Some(json!({"echo": "after reconnect"}))
    );
    client.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn rejection_of_a_reply_cannot_answer_a_request_with_the_same_id() -> anyhow::Result<()> {
    use std::sync::Arc;

    use codex_exec_server::RouteAwareHttpClient;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use rmcp::model::JsonRpcMessage;
    use rmcp::transport::streamable_http_client::StreamableHttpClient;
    use rmcp::transport::streamable_http_client::StreamableHttpPostResponse;
    use serde_json::json;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;

    let rejection = json!({
        "jsonrpc": "2.0", "id": 7,
        "error": {"code": -32600, "message": "unknown request"},
    });
    let server = MockServer::start().await;
    for (path, response) in [
        (
            "/reject",
            ResponseTemplate::new(409).set_body_json(&rejection),
        ),
        (
            "/json",
            ResponseTemplate::new(200).set_body_json(&rejection),
        ),
        (
            "/sse",
            ResponseTemplate::new(200).set_body_raw(
                format!("event: message\ndata: {rejection}\n\n"),
                "text/event-stream",
            ),
        ),
        ("/ack", ResponseTemplate::new(202)),
        ("/empty", ResponseTemplate::new(204)),
    ] {
        Mock::given(method("POST"))
            .and(wiremock::matchers::path(path))
            .respond_with(response)
            .mount(&server)
            .await;
    }
    let adapter = super::StreamableHttpClientAdapter::new(
        Arc::new(RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        ))),
        HeaderMap::new(),
        /*auth_provider*/ None,
        /*has_configured_headers*/ false,
        StreamableHttpRedirectMode::Legacy,
        Default::default(),
    );
    let post = |path: &str, message| {
        adapter.post_message(
            format!("{}{path}", server.uri()).into(),
            message,
            Some("same-session".into()),
            /*auth_token*/ None,
            Default::default(),
        )
    };
    let reply: rmcp::model::ClientJsonRpcMessage = serde_json::from_value(json!({
        "jsonrpc": "2.0", "id": 7, "result": {"action": "accept"},
    }))?;
    // The two peers number their requests independently. Only a POST carrying
    // our request may deliver the remote error to our pending request ID 7.
    let (request, rejected, json, sse) = tokio::join!(
        post(
            "/reject",
            serde_json::from_value(json!({
                "jsonrpc": "2.0", "id": 7, "method": "ping",
            }))?
        ),
        post("/reject", reply.clone()),
        post("/json", reply.clone()),
        post("/sse", reply.clone()),
    );
    let StreamableHttpPostResponse::Json(error @ JsonRpcMessage::Error(_), _) = request? else {
        anyhow::bail!("request lost its JSON-RPC error");
    };
    assert_eq!(serde_json::to_value(error)?, rejection);
    assert_eq!(
        [rejected.is_err(), json.is_err(), sse.is_err()],
        [true; 3],
        "409 JSON, 200 JSON, and 200 SSE must not dispatch messages for a reply"
    );
    for path in ["/ack", "/empty"] {
        assert!(matches!(
            post(path, reply.clone()).await?,
            StreamableHttpPostResponse::Accepted
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reply404Case {
    Reconnect,
    UnboundedInitializedFailure,
    ReconnectTimeout,
    ReplacementExpired,
    InitializedTimeout,
    InitializeSseTimeout,
    InitializeSseSuccess,
    SaturatedShutdown,
}

struct FloodGateHttpClient {
    inner: codex_exec_server::RouteAwareHttpClient,
    started: tokio::sync::Notify,
    release: tokio_util::sync::CancellationToken,
    expiry_started: tokio::sync::Notify,
    expiry_release: tokio_util::sync::CancellationToken,
    stall_initialize_sse: bool,
}

impl codex_exec_server::HttpClient for FloodGateHttpClient {
    fn http_request(
        &self,
        params: codex_exec_server::HttpRequestParams,
    ) -> futures::future::BoxFuture<
        '_,
        Result<codex_exec_server::HttpRequestResponse, codex_exec_server::ExecServerError>,
    > {
        codex_exec_server::HttpClient::http_request(&self.inner, params)
    }

    fn http_request_stream(
        &self,
        params: codex_exec_server::HttpRequestParams,
    ) -> futures::future::BoxFuture<
        '_,
        Result<
            (
                codex_exec_server::HttpRequestResponse,
                codex_exec_server::HttpResponseBodyStream,
            ),
            codex_exec_server::ExecServerError,
        >,
    > {
        let body = params
            .body
            .as_ref()
            .and_then(|body| serde_json::from_slice::<serde_json::Value>(&body.0).ok());
        let flood = body.as_ref().is_some_and(|body| {
            body["method"] == "tools/call" && body["params"]["name"] == "flood"
        });
        let fresh = body.as_ref().is_some_and(|body| {
            body["method"] == "tools/call" && body["params"]["name"] == "fresh"
        });
        let expiry = body
            .as_ref()
            .is_some_and(|body| body.get("method").is_none() && body["id"] == "prompt-1");
        let initialize = body
            .as_ref()
            .is_some_and(|body| body["method"] == "initialize");
        Box::pin(async move {
            let pending_params = (self.stall_initialize_sse && initialize).then(|| params.clone());
            let mut response =
                codex_exec_server::HttpClient::http_request_stream(&self.inner, params).await?;
            if let Some(mut pending_params) = pending_params
                && response.0.headers.iter().any(|header| {
                    header.name.eq_ignore_ascii_case("mcp-session-id")
                        && header.value == "new-session"
                })
            {
                use tokio::io::AsyncWriteExt as _;
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                pending_params.url = format!("http://{}", listener.local_addr().unwrap());
                // Only the recovery deadline should terminate this stalled body.
                pending_params.timeout_ms = None;
                tokio::spawn(async move {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    socket
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n")
                        .await
                        .unwrap();
                    let _ = tokio::io::copy(&mut socket, &mut tokio::io::sink()).await;
                });
                response.1 =
                    codex_exec_server::HttpClient::http_request_stream(&self.inner, pending_params)
                        .await?
                        .1;
            }
            if expiry && !self.expiry_release.is_cancelled() {
                self.expiry_started.notify_one();
                self.expiry_release.cancelled().await;
            }
            if flood || (fresh && !self.release.is_cancelled()) {
                self.started.notify_one();
                self.release.cancelled().await;
            }
            Ok(response)
        })
    }
}

#[tokio::test]
async fn reply_404_closes_two_calls_with_a_full_inbound_queue() -> anyhow::Result<()> {
    reply_404_recovery(Reply404Case::SaturatedShutdown).await
}

#[tokio::test]
async fn reply_404_closes_parent_without_replay() -> anyhow::Result<()> {
    reply_404_recovery(Reply404Case::Reconnect).await
}

#[tokio::test]
async fn unbounded_recovery_cleans_up_failed_initialization() -> anyhow::Result<()> {
    reply_404_recovery(Reply404Case::UnboundedInitializedFailure).await
}

#[tokio::test]
async fn fresh_tool_timeout_includes_session_recovery_handshake() -> anyhow::Result<()> {
    reply_404_recovery(Reply404Case::ReconnectTimeout).await
}

#[tokio::test]
async fn replacement_404_fails_without_another_reconnect() -> anyhow::Result<()> {
    reply_404_recovery(Reply404Case::ReplacementExpired).await
}

#[tokio::test]
async fn cancelled_initialized_notification_deletes_allocated_session() -> anyhow::Result<()> {
    reply_404_recovery(Reply404Case::InitializedTimeout).await
}

#[tokio::test]
async fn cancelled_recovery_initialize_sse_deletes_allocated_session() -> anyhow::Result<()> {
    reply_404_recovery(Reply404Case::InitializeSseTimeout).await
}

#[tokio::test]
async fn healthy_recovery_initialize_sse_keeps_allocated_session() -> anyhow::Result<()> {
    reply_404_recovery(Reply404Case::InitializeSseSuccess).await
}

async fn reply_404_recovery(case: Reply404Case) -> anyhow::Result<()> {
    use anyhow::Context as _;
    use codex_exec_server::RouteAwareHttpClient;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use rmcp::model::ClientCapabilities;
    use rmcp::model::ElicitationAction;
    use rmcp::model::Implementation;
    use rmcp::model::InitializeRequestParams;
    use rmcp::model::ProtocolVersion;
    use serde_json::Value;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use std::time::Duration;
    use tokio::sync::Notify;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;
    use tokio::time::timeout;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::Request;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let server = MockServer::start().await;
    let initializations = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&initializations);
    let slow_started = Arc::new(Notify::new());
    let on_slow = Arc::clone(&slow_started);
    let sibling_started = Arc::new(Notify::new());
    let on_sibling = Arc::clone(&sibling_started);
    Mock::given(method("POST")).and(path("/mcp")).respond_with(move |request: &Request| {
        let body: Value = request.body_json().unwrap();
        let session = request.headers.get("mcp-session-id").and_then(|value| value.to_str().ok());
        match body["method"].as_str() {
            Some("initialize") => {
                let n = count.fetch_add(1, Ordering::Relaxed);
                let body = json!({
                    "jsonrpc":"2.0", "id":body["id"], "result":{
                        "protocolVersion":body["params"]["protocolVersion"], "capabilities":{"tools":{}}, "serverInfo":{"name":"fixture","version":"1"}
                    }
                });
                let result = ResponseTemplate::new(200).insert_header("mcp-session-id", if n == 0 {"old-session"} else if n > 1 && case == Reply404Case::InitializeSseTimeout {"followup-session"} else {"new-session"});
                let result = if n == 1 && matches!(case, Reply404Case::InitializeSseTimeout | Reply404Case::InitializeSseSuccess) { result.set_body_raw(format!("event: message\ndata: {body}\n\n"), "text/event-stream") } else { result.set_body_json(body) };
                if n == 1 && case == Reply404Case::ReconnectTimeout { result.set_delay(Duration::from_secs(2)) } else { result }
            }
            Some("notifications/initialized") if count.load(Ordering::Relaxed) == 2 && case == Reply404Case::InitializedTimeout => ResponseTemplate::new(202).set_delay(Duration::from_secs(2)),
            Some("notifications/initialized") if count.load(Ordering::Relaxed) == 2 && case == Reply404Case::UnboundedInitializedFailure => ResponseTemplate::new(200).set_body_raw("", "text/event-stream"),
            Some("notifications/initialized") => ResponseTemplate::new(202),
            Some("tools/call") if body["params"]["name"] == "wait" => {
                let prompt = json!({"jsonrpc":"2.0","id":"prompt-1","method":"elicitation/create","params":{
                    "mode":"form","message":"Continue?","requestedSchema":{"type":"object","properties":{}}
                }});
                let mut events = format!("retry: 1\nid: cursor-1\nevent: message\ndata: {prompt}\n\n");
                if case == Reply404Case::SaturatedShutdown {
                    let mut second = prompt;
                    second["id"] = json!("prompt-2");
                    events.push_str(&format!("event: message\ndata: {second}\n\n"));
                }
                ResponseTemplate::new(200).set_body_raw(events,"text/event-stream")
            }
            Some("tools/call") if body["params"]["name"] == "sibling" => {
                on_sibling.notify_one();
                ResponseTemplate::new(200).set_delay(Duration::from_secs(30))
            }
            Some("tools/call") if body["params"]["name"] == "flood" => {
                let event = json!({"jsonrpc":"2.0","method":"notifications/tools/list_changed"});
                ResponseTemplate::new(200).set_body_raw(format!("event: message\ndata: {event}\n\n").repeat(64),"text/event-stream")
            }
            Some("tools/call") if session == Some("old-session") || (case == Reply404Case::ReplacementExpired && body["params"]["name"] == "fresh") => ResponseTemplate::new(404),
            Some("tools/call") => ResponseTemplate::new(200).set_body_json(json!({"jsonrpc":"2.0","id":body["id"],"result":{"content":[{"type":"text","text":"fresh succeeded"}]}})),
            None if body["id"] == "prompt-1" => ResponseTemplate::new(404),
            None if body["id"] == "prompt-2" => {
                on_slow.notify_one();
                ResponseTemplate::new(202).set_delay(Duration::from_secs(30))
            }
            _ => ResponseTemplate::new(400),
        }
    }).mount(&server).await;
    let resumed = Arc::new(Notify::new());
    let on_resume = Arc::clone(&resumed);
    let resumes = AtomicUsize::new(0);
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            if request
                .headers
                .get("last-event-id")
                .is_some_and(|id| id == "cursor-1")
            {
                if resumes.fetch_add(1, Ordering::Relaxed) > 0 {
                    on_resume.notify_one();
                }
                ResponseTemplate::new(404)
            } else {
                ResponseTemplate::new(405)
            }
        })
        .mount(&server)
        .await;
    let deleted = Arc::new(Notify::new());
    let old_deleted = Arc::new(Notify::new());
    let on_delete = Arc::clone(&deleted);
    let on_old_delete = Arc::clone(&old_deleted);
    Mock::given(method("DELETE"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            if request
                .headers
                .get("mcp-session-id")
                .is_some_and(|id| id == "new-session")
            {
                on_delete.notify_one();
            } else {
                on_old_delete.notify_one();
            }
            ResponseTemplate::new(200)
        })
        .mount(&server)
        .await;

    let http = Arc::new(FloodGateHttpClient {
        inner: RouteAwareHttpClient::new(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        )),
        started: Notify::new(),
        release: Default::default(),
        expiry_started: Notify::new(),
        expiry_release: Default::default(),
        stall_initialize_sse: case == Reply404Case::InitializeSseTimeout,
    });
    if case != Reply404Case::SaturatedShutdown {
        http.expiry_release.cancel();
    }
    if !matches!(
        case,
        Reply404Case::SaturatedShutdown | Reply404Case::ReplacementExpired
    ) {
        http.release.cancel();
    }
    let client = Arc::new(
        crate::RmcpClient::new_streamable_http_client(
            "fixture",
            &format!("{}/mcp", server.uri()),
            Some("fixture-token".to_string()),
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            Default::default(),
            Default::default(),
            http.clone(),
            /*auth_provider*/ None,
        )
        .await?,
    );
    let (route_tx, mut route_rx) = mpsc::unbounded_channel();
    client
        .initialize(
            InitializeRequestParams::new(
                ClientCapabilities::default(),
                Implementation::new("codex-test", "1"),
            )
            .with_protocol_version(ProtocolVersion::V_2025_11_25),
            Some(Duration::from_secs(5)),
            Box::new(move |_, _| {
                let (tx, rx) = oneshot::channel();
                let (ready_tx, ready_rx) = oneshot::channel();
                route_tx.send((tx, ready_rx)).unwrap();
                Box::pin(async move {
                    let response = rx.await?;
                    let _ = ready_tx.send(());
                    Ok(response)
                })
            }),
        )
        .await?;
    let parent_client = Arc::clone(&client);
    let parent = tokio::spawn(async move {
        parent_client
            .call_tool(
                "wait".into(),
                /*arguments*/ None,
                |_| Ok(None),
                Some(Duration::from_secs(
                    if case == Reply404Case::SaturatedShutdown {
                        20
                    } else {
                        5
                    },
                )),
            )
            .await
    });
    let (reply, _) = timeout(Duration::from_secs(3), route_rx.recv())
        .await?
        .expect("elicitation reached user");
    timeout(Duration::from_secs(3), resumed.notified()).await?;
    assert!(
        !parent.is_finished(),
        "resume 404 must not complete the parent before the user replies"
    );
    let (flood, second) = if case == Reply404Case::SaturatedShutdown {
        let second = timeout(Duration::from_secs(3), route_rx.recv())
            .await
            .context("second elicitation was not routed")?
            .expect("second elicitation reached user");
        let flood_client = Arc::clone(&client);
        let flood = tokio::spawn(async move {
            flood_client
                .call_tool(
                    "flood".into(),
                    /*arguments*/ None,
                    |_| Ok(None),
                    Some(Duration::from_secs(20)),
                )
                .await
        });
        timeout(Duration::from_secs(3), http.started.notified())
            .await
            .context("flood did not reach HTTP gate")?;
        (Some(flood), Some(second))
    } else {
        (None, None)
    };
    reply
        .send(crate::ElicitationResponse {
            action: ElicitationAction::Accept,
            content: Some(json!({})),
            meta: None,
        })
        .unwrap();
    if let Some((second_reply, ready)) = second {
        timeout(Duration::from_secs(3), http.expiry_started.notified())
            .await
            .context("first reply POST did not reach expiry gate")?;
        second_reply
            .send(crate::ElicitationResponse {
                action: ElicitationAction::Accept,
                content: Some(json!({})),
                meta: None,
            })
            .unwrap();
        timeout(Duration::from_secs(3), ready)
            .await
            .context("second response handler did not finish")??;
        http.expiry_release.cancel();
        timeout(Duration::from_secs(3), slow_started.notified())
            .await
            .context("second reply POST did not start")?;
        timeout(Duration::from_secs(3), async {
            while !client.is_closed().await {
                tokio::task::yield_now().await;
            }
        })
        .await
        .context("reply 404 did not mark session closed")?;
        // The service no longer receives; 64 events fill the SDK's 16-message inbound queue while the stalled reply drains.
        http.release.cancel();
    }
    let error = timeout(
        Duration::from_secs(if flood.is_some() { 9 } else { 3 }),
        parent,
    )
    .await??
    .expect_err("expired reply cannot replay the original call");
    assert_eq!(crate::request_error_code(&error), Some("transport_closed"));
    if let Some(flood) = flood {
        let error = timeout(Duration::from_secs(1), flood)
            .await??
            .expect_err("concurrent old-session call must fail");
        assert_eq!(crate::request_error_code(&error), Some("transport_closed"));
        timeout(Duration::from_secs(2), old_deleted.notified())
            .await
            .expect("cancelled worker must release the old session");
        client.shutdown().await;
        return Ok(());
    }

    let budget = if case == Reply404Case::InitializeSseTimeout {
        Duration::from_secs(1)
    } else if matches!(
        case,
        Reply404Case::ReconnectTimeout | Reply404Case::InitializedTimeout
    ) {
        Duration::from_millis(250)
    } else {
        Duration::from_secs(2)
    };
    let fresh_client = Arc::clone(&client);
    let fresh = tokio::spawn(async move {
        fresh_client
            .call_tool(
                "fresh".into(),
                /*arguments*/ None,
                |_| Ok(None),
                (case != Reply404Case::UnboundedInitializedFailure).then_some(budget),
            )
            .await
    });
    let sibling = if case == Reply404Case::ReplacementExpired {
        timeout(Duration::from_secs(3), http.started.notified()).await?;
        let sibling_client = Arc::clone(&client);
        let sibling = tokio::spawn(async move {
            sibling_client
                .call_tool(
                    "sibling".into(),
                    /*arguments*/ None,
                    |_| Ok(None),
                    /*timeout*/ None,
                )
                .await
        });
        timeout(Duration::from_secs(3), sibling_started.notified()).await?;
        http.release.cancel();
        Some(sibling)
    } else {
        None
    };
    let fresh = timeout(Duration::from_secs(3), fresh).await??;
    match case {
        Reply404Case::Reconnect
        | Reply404Case::UnboundedInitializedFailure
        | Reply404Case::InitializeSseSuccess => {
            assert_eq!(
                fresh?.content,
                vec![rmcp::model::ContentBlock::text("fresh succeeded")]
            );
        }
        Reply404Case::ReconnectTimeout
        | Reply404Case::InitializedTimeout
        | Reply404Case::InitializeSseTimeout => {
            assert_eq!(
                crate::request_error_code(
                    &fresh.expect_err("recovery must respect fresh call timeout")
                ),
                Some("request_timeout")
            );
        }
        Reply404Case::SaturatedShutdown => unreachable!(),
        Reply404Case::ReplacementExpired => {
            assert_eq!(
                crate::request_error_code(&fresh.expect_err("replacement expired")),
                Some("transport_error")
            );
            assert_eq!(
                initializations.load(Ordering::Relaxed),
                2,
                "fresh call must reconnect only once"
            );
        }
    }
    if let Some(sibling) = sibling {
        let error = timeout(Duration::from_secs(8), sibling)
            .await
            .context("expired replacement left an unbounded sibling pending")??
            .expect_err("expired replacement must fail sibling");
        assert_eq!(crate::request_error_code(&error), Some("transport_closed"));
    }
    if matches!(
        case,
        Reply404Case::InitializedTimeout
            | Reply404Case::UnboundedInitializedFailure
            | Reply404Case::InitializeSseTimeout
    ) {
        timeout(Duration::from_secs(2), deleted.notified())
            .await
            .expect("allocated session DELETE must be attempted");
    }
    if !matches!(
        case,
        Reply404Case::Reconnect
            | Reply404Case::UnboundedInitializedFailure
            | Reply404Case::InitializeSseSuccess
    ) {
        let followup = timeout(
            Duration::from_secs(1),
            client.call_tool(
                "followup".into(),
                /*arguments*/ None,
                |_| Ok(None),
                Some(Duration::from_secs(2)),
            ),
        )
        .await
        .expect("recovery must release worker and semaphore")?;
        assert_eq!(
            followup.content,
            vec![rmcp::model::ContentBlock::text("fresh succeeded")]
        );
    }
    let requests = server.received_requests().await.unwrap();
    let posts = requests
        .iter()
        .filter(|r| r.method == "POST")
        .map(|r| (r, r.body_json::<Value>().unwrap()))
        .collect::<Vec<_>>();
    let replies = posts
        .iter()
        .filter(|(_, body)| body.get("method").is_none())
        .collect::<Vec<_>>();
    assert_eq!(
        replies.len(),
        1,
        "no other prompt replies or approval replays"
    );
    assert_eq!(replies[0].0.headers["mcp-session-id"], "old-session");
    assert_eq!(replies[0].1["id"], "prompt-1");
    assert_eq!(replies[0].1["result"]["action"], "accept");
    assert_eq!(
        posts
            .iter()
            .filter(|(_, b)| b["method"] == "tools/call" && b["params"]["name"] == "wait")
            .count(),
        1
    );
    assert_eq!(
        posts
            .iter()
            .filter(|(_, b)| b["method"] == "tools/call" && b["params"]["name"] == "fresh")
            .count(),
        usize::from(matches!(
            case,
            Reply404Case::Reconnect
                | Reply404Case::UnboundedInitializedFailure
                | Reply404Case::ReplacementExpired
                | Reply404Case::InitializeSseSuccess
        ))
    );
    assert_eq!(
        initializations.load(Ordering::Relaxed),
        if matches!(
            case,
            Reply404Case::Reconnect | Reply404Case::InitializeSseSuccess
        ) {
            2
        } else {
            3
        }
    );
    if matches!(
        case,
        Reply404Case::InitializedTimeout
            | Reply404Case::UnboundedInitializedFailure
            | Reply404Case::InitializeSseTimeout
    ) {
        let deletes = requests
            .iter()
            .filter(|r| {
                r.method == "DELETE"
                    && r.headers
                        .get("mcp-session-id")
                        .is_some_and(|id| id == "new-session")
            })
            .collect::<Vec<_>>();
        assert_eq!(deletes.len(), 1);
        assert_eq!(deletes[0].headers["authorization"], "Bearer fixture-token");
    }
    if case == Reply404Case::InitializeSseSuccess {
        assert!(
            !requests.iter().any(|request| request.method == "DELETE"
                && request
                    .headers
                    .get("mcp-session-id")
                    .is_some_and(|id| id == "new-session")),
            "healthy SSE initialization must not delete the active session"
        );
    }
    client.shutdown().await;
    Ok(())
}
