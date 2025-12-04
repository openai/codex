#![allow(clippy::expect_used)]

use async_trait::async_trait;
use codex_api::ModelsClient;
use codex_api::auth::AuthProvider;
use codex_api::provider::Provider;
use codex_api::provider::RetryConfig;
use codex_api::provider::WireApi;
use codex_client::HttpTransport;
use codex_client::Request;
use codex_client::Response;
use codex_client::StreamResponse;
use codex_client::TransportError;
use codex_protocol::openai_models::ModelsResponse;
use http::HeaderMap;
use http::StatusCode;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Clone, Default)]
struct DummyAuth;

impl AuthProvider for DummyAuth {
    fn bearer_token(&self) -> Option<String> {
        None
    }
}

#[derive(Clone)]
struct CapturingTransport {
    last_request: Arc<Mutex<Option<Request>>>,
    body: Arc<ModelsResponse>,
}

#[async_trait]
impl HttpTransport for CapturingTransport {
    async fn execute(&self, req: Request) -> Result<Response, TransportError> {
        *self.last_request.lock().expect("lock poisoned") = Some(req);
        let body = serde_json::to_vec(&*self.body).expect("serialization should succeed");
        Ok(Response {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: body.into(),
        })
    }

    async fn stream(&self, _req: Request) -> Result<StreamResponse, TransportError> {
        Err(TransportError::Build("stream should not run".to_string()))
    }
}

fn provider(base_url: &str) -> Provider {
    Provider {
        name: "test".to_string(),
        base_url: base_url.to_string(),
        query_params: None,
        wire: WireApi::Responses,
        headers: HeaderMap::new(),
        retry: RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(1),
            retry_429: false,
            retry_5xx: true,
            retry_transport: true,
        },
        stream_idle_timeout: Duration::from_secs(1),
    }
}

#[tokio::test]
async fn builds_correct_url() {
    let response = ModelsResponse { models: Vec::new() };
    let transport = CapturingTransport {
        last_request: Arc::new(Mutex::new(None)),
        body: Arc::new(response),
    };

    let client = ModelsClient::new(
        transport.clone(),
        provider("https://example.com/api/codex"),
        DummyAuth,
    );

    client
        .list_models("0.1.2", HeaderMap::new())
        .await
        .expect("list_models should succeed");

    let url = transport
        .last_request
        .lock()
        .expect("lock poisoned")
        .as_ref()
        .expect("request recorded")
        .url
        .clone();

    assert_eq!(
        url,
        "https://example.com/api/codex/models?client_version=0.1.2"
    );
}
