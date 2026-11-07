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
