//! Private connector results must reach the wire without entering native frame logs.

#![allow(clippy::expect_used)]

use std::net::TcpListener;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::thread::ThreadId;
use std::time::Duration;

use anyhow::Result;
use codex_api::AuthProvider;
use codex_api::Provider;
use codex_api::ResponseCreateWsRequest;
use codex_api::ResponseEvent;
use codex_api::ResponsesWebsocketClient;
use codex_api::ResponsesWsRequest;
use codex_api::RetryConfig;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_protocol::models::ExecutedToolCall;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::ToolResultBody;
use futures::StreamExt;
use http::HeaderMap;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

struct NoAuth;

impl AuthProvider for NoAuth {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) -> Result<(), codex_api::AuthError> {
        Ok(())
    }
}

struct ClientLogs {
    thread: ThreadId,
    records: Mutex<Vec<(String, String)>>,
}

impl log::Log for ClientLogs {
    fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        // The mock server receives private bytes too; only inspect the client's diagnostics.
        if std::thread::current().id() == self.thread {
            self.records
                .lock()
                .expect("log recorder lock")
                .push((record.target().to_owned(), record.args().to_string()));
        }
    }

    fn flush(&self) {}
}

#[tokio::test(flavor = "current_thread")]
async fn connector_results_reach_websocket_without_payload_logs() -> Result<()> {
    // Keep this in its own integration binary: log's logger is process-global.
    static LOGS: OnceLock<ClientLogs> = OnceLock::new();
    let logs = LOGS.get_or_init(|| ClientLogs {
        thread: std::thread::current().id(),
        records: Mutex::new(Vec::new()),
    });
    log::set_logger(logs).expect("install independent log-facade recorder");
    log::set_max_level(log::LevelFilter::Trace);
    tracing::dispatcher::with_default(&tracing::Dispatch::none(), || {
        log::trace!("independent logger control");
    });

    const TIMEOUT: Duration = Duration::from_secs(5);
    const RESULT: &str = "PRIVATE_CONNECTOR_FRAME_RESULT";
    const CALL_ID: &str = "exec-abcd1234-bcde-4123-9abc-123456789abc";
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let server = std::thread::spawn(move || -> Result<Value> {
        let (stream, _) = listener.accept()?;
        stream.set_read_timeout(Some(TIMEOUT))?;
        stream.set_write_timeout(Some(TIMEOUT))?;
        // Decline optional compression so leaked payload bytes remain recognizable.
        let mut socket = tungstenite::accept_with_config(
            stream,
            Some(tungstenite::protocol::WebSocketConfig::default()),
        )?;
        let request = serde_json::from_str(&socket.read()?.into_text()?)?;
        socket.send(tungstenite::Message::text(
            json!({"type": "response.completed", "response": {"id": "resp_test"}}).to_string(),
        ))?;
        Ok(request)
    });
    let client = ResponsesWebsocketClient::new(
        Provider {
            name: "logging fixture".into(),
            base_url: format!("http://{address}"),
            query_params: None,
            headers: HeaderMap::new(),
            retry: RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(/*millis*/ 1),
                retry_429: false,
                retry_5xx: false,
                retry_transport: false,
            },
            stream_idle_timeout: TIMEOUT,
        },
        Arc::new(NoAuth),
    );
    let connection = tokio::time::timeout(
        TIMEOUT,
        client.connect(
            &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            HeaderMap::new(),
            HeaderMap::new(),
            /*turn_state*/ None,
            /*telemetry*/ None,
        ),
    )
    .await??;
    let mut item: ResponseItem = serde_json::from_value(json!({
        "type": "function_call_output", "call_id": "outer", "output": "one match",
    }))?;
    item.append_executed_tool_calls(vec![ExecutedToolCall::new(
        "mcp__codex_apps__search".into(),
        json!({}),
    )]);
    let result = json!({"content": [{"type": "text", "text": RESULT}]});
    let mut body = ToolResultBody::from_mcp_result(&serde_json::from_value(result.clone())?)
        .expect("eligible connector result");
    body.set_nested_call_id(CALL_ID);
    let input = [item];
    let request = ResponsesWsRequest::ResponseCreate(ResponseCreateWsRequest {
        model: "gpt-test",
        stream: true,
        service_tier: None,
        previous_response_id: None,
        input: &input,
        tool_results: vec![(0, 0, Arc::new(body))],
        tools: None,
        tool_choice: "auto",
        parallel_tool_calls: false,
        reasoning: None,
        store: false,
        stream_options: None,
        include: &[],
        prompt_cache_key: None,
        prompt_cache_options: None,
        text: None,
        generate: None,
        client_metadata: None,
        access_programs: None,
        request_country: &Default::default(),
    });
    let canonical = serde_json::to_value(&request)?;
    let mut expected = canonical.clone();
    let call = &mut expected["input"][0]["internal_chat_message_metadata_passthrough"]["executed_tool_calls"]
        [0];
    call["tool_result"] = result;
    call["tool_result_call_id"] = json!(CALL_ID);
    let mut observed = bytes::Bytes::new();
    let mut events = connection
        .stream_request_with_observer(
            request,
            /*connection_reused*/ false,
            /*turn_state*/ None,
            |json| observed = json.clone(),
        )
        .await?;
    assert_eq!(serde_json::from_slice::<Value>(&observed)?, canonical);
    let completed = tokio::time::timeout(TIMEOUT, async {
        let mut completed = false;
        while let Some(event) = events.next().await {
            completed |= matches!(event?, ResponseEvent::Completed { .. });
        }
        Ok::<_, codex_api::ApiError>(completed)
    })
    .await??;
    assert!(completed, "native client must receive completion");
    assert_eq!(server.join().expect("server thread")?, expected);

    let records = logs.records.lock().expect("log recorder lock");
    assert!(
        records
            .iter()
            .any(|(_, text)| text == "independent logger control")
    );
    for (target, prefix) in [
        ("tungstenite::protocol", "Sending frame"),
        ("tungstenite::protocol::frame", "writing frame"),
    ] {
        assert!(
            records
                .iter()
                .any(|(name, text)| name == target && text.starts_with(prefix))
        );
    }
    let diagnostics = format!("{records:?}");
    for marker in [RESULT, CALL_ID] {
        let hex: String = marker.bytes().map(|byte| format!("{byte:02x}")).collect();
        assert!(!diagnostics.contains(marker), "frame log exposed plaintext");
        assert!(!diagnostics.contains(&hex), "frame log exposed hex payload");
    }
    Ok(())
}
