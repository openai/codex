use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use codex_api::AuthProvider;
use codex_api::Provider;
use codex_api::ResponsesApiRequest;
use codex_api::ResponsesClient;
use codex_api::ResponsesOptions;
use codex_api::requests::responses::Compression;
use codex_client::HttpTransport;
use codex_client::Request;
use codex_client::Response;
use codex_client::StreamResponse;
use codex_client::TransportError;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use http::HeaderMap;
use http::HeaderValue;
use http::StatusCode;
use pretty_assertions::assert_eq;

fn assert_path_ends_with(requests: &[Request], suffix: &str) {
    assert_eq!(requests.len(), 1);
    let url = &requests[0].url;
    assert!(
        url.ends_with(suffix),
        "expected url to end with {suffix}, got {url}"
    );
}

#[derive(Debug, Default, Clone)]
struct RecordingState {
    stream_requests: Arc<Mutex<Vec<Request>>>,
}

impl RecordingState {
    fn record(&self, req: Request) {
        let mut guard = self
            .stream_requests
            .lock()
            .unwrap_or_else(|err| panic!("mutex poisoned: {err}"));
        guard.push(req);
    }

    fn take_stream_requests(&self) -> Vec<Request> {
        let mut guard = self
            .stream_requests
            .lock()
            .unwrap_or_else(|err| panic!("mutex poisoned: {err}"));
        std::mem::take(&mut *guard)
    }
}

#[derive(Clone)]
struct RecordingTransport {
    state: RecordingState,
}

impl RecordingTransport {
    fn new(state: RecordingState) -> Self {
        Self { state }
    }
}

#[async_trait]
impl HttpTransport for RecordingTransport {
    async fn execute(&self, _req: Request) -> Result<Response, TransportError> {
        Err(TransportError::Build("execute should not run".to_string()))
    }

    async fn stream(&self, req: Request) -> Result<StreamResponse, TransportError> {
        self.state.record(req);

        let stream = futures::stream::iter(Vec::<Result<Bytes, TransportError>>::new());
        Ok(StreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            bytes: Box::pin(stream),
        })
    }
}

#[derive(Clone, Default)]
struct NoAuth;

impl AuthProvider for NoAuth {
    fn bearer_token(&self) -> Option<String> {
        None
    }
}

#[derive(Clone)]
struct StaticAuth {
    token: String,
    account_id: String,
}

impl StaticAuth {
    fn new(token: &str, account_id: &str) -> Self {
        Self {
            token: token.to_string(),
            account_id: account_id.to_string(),
        }
    }
}

impl AuthProvider for StaticAuth {
    fn bearer_token(&self) -> Option<String> {
        Some(self.token.clone())
    }

    fn account_id(&self) -> Option<String> {
        Some(self.account_id.clone())
    }
}

fn provider(name: &str) -> Provider {
    Provider {
        name: name.to_string(),
        base_url: "https://example.com/v1".to_string(),
        query_params: None,
        headers: HeaderMap::new(),
        retry: codex_api::provider::RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(1),
            retry_429: false,
            retry_5xx: false,
            retry_transport: true,
        },
        stream_idle_timeout: Duration::from_millis(10),
    }
}

#[derive(Clone)]
struct FlakyTransport {
    state: Arc<Mutex<i64>>,
}

impl Default for FlakyTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl FlakyTransport {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(0)),
        }
    }

    fn attempts(&self) -> i64 {
        *self
            .state
            .lock()
            .unwrap_or_else(|err| panic!("mutex poisoned: {err}"))
    }
}

#[async_trait]
impl HttpTransport for FlakyTransport {
    async fn execute(&self, _req: Request) -> Result<Response, TransportError> {
        Err(TransportError::Build("execute should not run".to_string()))
    }

    async fn stream(&self, _req: Request) -> Result<StreamResponse, TransportError> {
        let mut attempts = self
            .state
            .lock()
            .unwrap_or_else(|err| panic!("mutex poisoned: {err}"));
        *attempts += 1;

        if *attempts == 1 {
            return Err(TransportError::Network("first attempt fails".to_string()));
        }

        let stream = futures::stream::iter(vec![Ok(Bytes::from(
            r#"event: message
data: {"id":"resp-1","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"hi"}]}]}

"#,
        ))]);

        Ok(StreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            bytes: Box::pin(stream),
        })
    }
}

#[tokio::test]
async fn responses_client_uses_responses_path() -> Result<()> {
    let state = RecordingState::default();
    let transport = RecordingTransport::new(state.clone());
    let client = ResponsesClient::new(transport, provider("openai"), NoAuth);

    let body = serde_json::json!({ "echo": true });
    let _stream = client
        .stream(body, HeaderMap::new(), Compression::None, None)
        .await?;

    let requests = state.take_stream_requests();
    assert_path_ends_with(&requests, "/responses");
    Ok(())
}

#[tokio::test]
async fn streaming_client_adds_auth_headers() -> Result<()> {
    let state = RecordingState::default();
    let transport = RecordingTransport::new(state.clone());
    let auth = StaticAuth::new("secret-token", "acct-1");
    let client = ResponsesClient::new(transport, provider("openai"), auth);

    let body = serde_json::json!({ "model": "gpt-test" });
    let _stream = client
        .stream(body, HeaderMap::new(), Compression::None, None)
        .await?;

    let requests = state.take_stream_requests();
    assert_eq!(requests.len(), 1);
    let req = &requests[0];

    let auth_header = req.headers.get(http::header::AUTHORIZATION);
    assert!(auth_header.is_some(), "missing auth header");
    assert_eq!(
        auth_header.unwrap().to_str().ok(),
        Some("Bearer secret-token")
    );

    let account_header = req.headers.get("ChatGPT-Account-ID");
    assert!(account_header.is_some(), "missing account header");
    assert_eq!(account_header.unwrap().to_str().ok(), Some("acct-1"));

    let accept_header = req.headers.get(http::header::ACCEPT);
    assert!(accept_header.is_some(), "missing Accept header");
    assert_eq!(
        accept_header.unwrap().to_str().ok(),
        Some("text/event-stream")
    );
    Ok(())
}

#[tokio::test]
async fn streaming_client_retries_on_transport_error() -> Result<()> {
    let transport = FlakyTransport::new();

    let mut provider = provider("openai");
    provider.retry.max_attempts = 2;

    let request = ResponsesApiRequest {
        model: "gpt-test".into(),
        instructions: "Say hi".into(),
        input: Vec::new(),
        tools: Vec::new(),
        tool_choice: "auto".into(),
        parallel_tool_calls: false,
        reasoning: None,
        store: false,
        stream: true,
        include: Vec::new(),
        prompt_cache_key: None,
        text: None,
    };
    let client = ResponsesClient::new(transport.clone(), provider, NoAuth);

    let _stream = client
        .stream_request(
            request,
            ResponsesOptions {
                compression: Compression::None,
                ..Default::default()
            },
        )
        .await?;
    assert_eq!(transport.attempts(), 2);
    Ok(())
}

#[tokio::test]
async fn azure_default_store_attaches_ids_and_headers() -> Result<()> {
    let state = RecordingState::default();
    let transport = RecordingTransport::new(state.clone());
    let client = ResponsesClient::new(transport, provider("azure"), NoAuth);

    let request = ResponsesApiRequest {
        model: "gpt-test".into(),
        instructions: "Say hi".into(),
        input: vec![ResponseItem::Message {
            id: Some("msg_1".into()),
            role: "user".into(),
            content: vec![ContentItem::InputText { text: "hi".into() }],
            end_turn: None,
            phase: None,
        }],
        tools: Vec::new(),
        tool_choice: "auto".into(),
        parallel_tool_calls: false,
        reasoning: None,
        store: true,
        stream: true,
        include: Vec::new(),
        prompt_cache_key: None,
        text: None,
    };

    let mut extra_headers = HeaderMap::new();
    extra_headers.insert("x-test-header", HeaderValue::from_static("present"));
    let _stream = client
        .stream_request(
            request,
            ResponsesOptions {
                conversation_id: Some("sess_123".into()),
                session_source: Some(SessionSource::SubAgent(SubAgentSource::Review)),
                extra_headers,
                compression: Compression::None,
                turn_state: None,
            },
        )
        .await?;

    let requests = state.take_stream_requests();
    assert_eq!(requests.len(), 1);
    let req = &requests[0];

    assert_eq!(
        req.headers.get("session_id").and_then(|v| v.to_str().ok()),
        Some("sess_123")
    );
    assert_eq!(
        req.headers
            .get("x-openai-subagent")
            .and_then(|v| v.to_str().ok()),
        Some("review")
    );
    assert_eq!(
        req.headers
            .get("x-test-header")
            .and_then(|v| v.to_str().ok()),
        Some("present")
    );

    let input_id = req
        .body
        .as_ref()
        .and_then(|body| body.get("input"))
        .and_then(|input| input.get(0))
        .and_then(|item| item.get("id"))
        .and_then(|id| id.as_str());
    assert_eq!(input_id, Some("msg_1"));

    Ok(())
}
