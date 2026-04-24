use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use codex_api::ApiError;
use codex_api::AuthError;
use codex_api::AuthProvider;
use codex_api::Compression;
use codex_api::Provider;
use codex_api::ResponsesApiRequest;
use codex_api::ResponsesClient;
use codex_api::ResponsesOptions;
use codex_client::HttpTransport;
use codex_client::Request;
use codex_client::RequestBody;
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
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}
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
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        let token = &self.token;
        if let Ok(header) = HeaderValue::from_str(&format!("Bearer {token}")) {
            headers.insert(http::header::AUTHORIZATION, header);
        }
        if let Ok(header) = HeaderValue::from_str(&self.account_id) {
            headers.insert("ChatGPT-Account-ID", header);
        }
    }
}

fn provider(name: &str) -> Provider {
    Provider {
        name: name.to_string(),
        base_url: "https://example.com/v1".to_string(),
        query_params: None,
        headers: HeaderMap::new(),
        retry: codex_api::RetryConfig {
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

#[derive(Clone)]
struct FailsOnceAuth {
    attempts: Arc<Mutex<i64>>,
    error: Arc<AuthError>,
}

impl FailsOnceAuth {
    fn transient() -> Self {
        Self {
            attempts: Arc::new(Mutex::new(0)),
            error: Arc::new(AuthError::Transient(
                "sts temporarily unavailable".to_string(),
            )),
        }
    }

    fn build() -> Self {
        Self {
            attempts: Arc::new(Mutex::new(0)),
            error: Arc::new(AuthError::Build("invalid auth configuration".to_string())),
        }
    }

    fn attempts(&self) -> i64 {
        *self
            .attempts
            .lock()
            .unwrap_or_else(|err| panic!("mutex poisoned: {err}"))
    }
}

#[async_trait]
impl AuthProvider for FailsOnceAuth {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    async fn apply_auth(&self, request: Request) -> Result<Request, AuthError> {
        let mut attempts = self
            .attempts
            .lock()
            .unwrap_or_else(|err| panic!("mutex poisoned: {err}"));
        *attempts += 1;

        if *attempts == 1 {
            return match self.error.as_ref() {
                AuthError::Build(message) => Err(AuthError::Build(message.clone())),
                AuthError::Transient(message) => Err(AuthError::Transient(message.clone())),
            };
        }

        Ok(request)
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
    let client = ResponsesClient::new(transport, provider("openai"), Arc::new(NoAuth));

    let body = serde_json::json!({ "echo": true });
    let _stream = client
        .stream(
            body,
            HeaderMap::new(),
            Compression::None,
            /*turn_state*/ None,
        )
        .await?;

    let requests = state.take_stream_requests();
    assert_path_ends_with(&requests, "/responses");
    Ok(())
}

#[tokio::test]
async fn streaming_client_adds_auth_headers() -> Result<()> {
    let state = RecordingState::default();
    let transport = RecordingTransport::new(state.clone());
    let auth = Arc::new(StaticAuth::new("secret-token", "acct-1"));
    let client = ResponsesClient::new(transport, provider("openai"), auth);

    let body = serde_json::json!({ "model": "gpt-test" });
    let _stream = client
        .stream(
            body,
            HeaderMap::new(),
            Compression::None,
            /*turn_state*/ None,
        )
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
        service_tier: None,
        prompt_cache_key: None,
        text: None,
        client_metadata: None,
    };
    let client = ResponsesClient::new(transport.clone(), provider, Arc::new(NoAuth));

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
async fn streaming_client_retries_on_transient_auth_error() -> Result<()> {
    let state = RecordingState::default();
    let transport = RecordingTransport::new(state.clone());
    let auth = FailsOnceAuth::transient();

    let mut provider = provider("openai");
    provider.retry.max_attempts = 2;

    let client = ResponsesClient::new(transport, provider, Arc::new(auth.clone()));
    let body = serde_json::json!({ "model": "gpt-test" });
    let _stream = client
        .stream(
            body,
            HeaderMap::new(),
            Compression::None,
            /*turn_state*/ None,
        )
        .await?;

    assert_eq!(auth.attempts(), 2);
    assert_eq!(state.take_stream_requests().len(), 1);
    Ok(())
}

#[tokio::test]
async fn streaming_client_does_not_retry_auth_build_error() -> Result<()> {
    let state = RecordingState::default();
    let transport = RecordingTransport::new(state.clone());
    let auth = FailsOnceAuth::build();

    let mut provider = provider("openai");
    provider.retry.max_attempts = 2;

    let client = ResponsesClient::new(transport, provider, Arc::new(auth.clone()));
    let body = serde_json::json!({ "model": "gpt-test" });
    let result = client
        .stream(
            body,
            HeaderMap::new(),
            Compression::None,
            /*turn_state*/ None,
        )
        .await;
    let err = match result {
        Ok(_) => panic!("auth build errors should fail without retry"),
        Err(err) => err,
    };

    assert!(matches!(
        err,
        ApiError::Transport(TransportError::Build(message))
            if message == "invalid auth configuration"
    ));
    assert_eq!(auth.attempts(), 1);
    assert_eq!(state.take_stream_requests().len(), 0);
    Ok(())
}

#[tokio::test]
async fn azure_default_store_attaches_ids_and_headers() -> Result<()> {
    let state = RecordingState::default();
    let transport = RecordingTransport::new(state.clone());
    let client = ResponsesClient::new(transport, provider("azure"), Arc::new(NoAuth));

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
        service_tier: None,
        prompt_cache_key: None,
        text: None,
        client_metadata: None,
    };

    let mut extra_headers = HeaderMap::new();
    extra_headers.insert("x-test-header", HeaderValue::from_static("present"));
    let _stream = client
        .stream_request(
            request,
            ResponsesOptions {
                session_id: Some("sess_123".into()),
                thread_id: Some("thread_456".into()),
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
        req.headers.get("thread_id").and_then(|v| v.to_str().ok()),
        Some("thread_456")
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
        .and_then(RequestBody::json)
        .and_then(|body| body.get("input"))
        .and_then(|input| input.get(0))
        .and_then(|item| item.get("id"))
        .and_then(|id| id.as_str());
    assert_eq!(input_id, Some("msg_1"));

    Ok(())
}
