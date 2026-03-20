use crate::auth::AuthProvider;
use crate::endpoint::realtime_websocket::RealtimeSessionConfig;
use crate::endpoint::realtime_websocket::session_update_session_json;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::provider::Provider;
use codex_client::HttpTransport;
use codex_client::RequestTelemetry;
use http::HeaderMap;
use http::Method;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;

pub struct RealtimeClientSecretsClient<T: HttpTransport, A: AuthProvider> {
    session: EndpointSession<T, A>,
}

impl<T: HttpTransport, A: AuthProvider> RealtimeClientSecretsClient<T, A> {
    pub fn new(transport: T, provider: Provider, auth: A) -> Self {
        Self {
            session: EndpointSession::new(transport, provider, auth),
        }
    }

    pub fn with_telemetry(self, request: Option<Arc<dyn RequestTelemetry>>) -> Self {
        Self {
            session: self.session.with_request_telemetry(request),
        }
    }

    fn path() -> &'static str {
        "codex/realtime/client_secrets"
    }

    pub async fn create(
        &self,
        config: &RealtimeSessionConfig,
        extra_headers: HeaderMap,
    ) -> Result<String, ApiError> {
        let body = realtime_client_secret_request_body(config)?;
        let resp = self
            .session
            .execute(Method::POST, Self::path(), extra_headers, Some(body))
            .await?;
        let parsed: RealtimeClientSecretResponse =
            serde_json::from_slice(&resp.body).map_err(|err| {
                ApiError::Stream(format!(
                    "failed to decode realtime client secret response: {err}"
                ))
            })?;
        if parsed.value.trim().is_empty() {
            return Err(ApiError::Stream(
                "realtime client secret response was missing a value".to_string(),
            ));
        }
        Ok(parsed.value)
    }
}

fn realtime_client_secret_request_body(config: &RealtimeSessionConfig) -> Result<Value, ApiError> {
    let mut session = session_update_session_json(
        config.event_parser,
        config.instructions.clone(),
        config.session_mode,
    )?;
    if let Some(model) = config.model.as_ref()
        && let Some(session_object) = session.as_object_mut()
    {
        session_object.insert("model".to_string(), Value::String(model.clone()));
    }

    Ok(json!({
        "session": session,
    }))
}

#[derive(Debug, Deserialize)]
struct RealtimeClientSecretResponse {
    value: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::RetryConfig;
    use async_trait::async_trait;
    use codex_client::Request;
    use codex_client::Response;
    use codex_client::StreamResponse;
    use codex_client::TransportError;
    use http::HeaderMap;
    use http::Method;
    use http::StatusCode;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    #[derive(Clone)]
    struct CapturingTransport {
        last_request: Arc<Mutex<Option<Request>>>,
        response_body: Arc<Vec<u8>>,
    }

    impl CapturingTransport {
        fn new(response_body: Vec<u8>) -> Self {
            Self {
                last_request: Arc::new(Mutex::new(None)),
                response_body: Arc::new(response_body),
            }
        }
    }

    #[async_trait]
    impl HttpTransport for CapturingTransport {
        async fn execute(&self, req: Request) -> Result<Response, TransportError> {
            *self.last_request.lock().expect("lock request store") = Some(req);
            Ok(Response {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: self.response_body.as_ref().clone().into(),
            })
        }

        async fn stream(&self, _req: Request) -> Result<StreamResponse, TransportError> {
            Err(TransportError::Build("stream should not run".to_string()))
        }
    }

    #[derive(Clone, Default)]
    struct DummyAuth;

    impl AuthProvider for DummyAuth {
        fn bearer_token(&self) -> Option<String> {
            None
        }
    }

    fn provider(base_url: &str) -> Provider {
        Provider {
            name: "test".to_string(),
            base_url: base_url.to_string(),
            query_params: None,
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
    async fn create_posts_expected_payload_and_parses_value() {
        let transport = CapturingTransport::new(
            serde_json::to_vec(&json!({
                "value": "ek-test-secret"
            }))
            .expect("serialize response"),
        );
        let client = RealtimeClientSecretsClient::new(
            transport.clone(),
            provider("https://example.com/backend-api"),
            DummyAuth,
        );
        let session = RealtimeSessionConfig {
            instructions: "Be helpful".to_string(),
            model: Some("gpt-realtime".to_string()),
            session_id: Some("session-1".to_string()),
            event_parser: crate::endpoint::realtime_websocket::RealtimeEventParser::RealtimeV2,
            session_mode: crate::endpoint::realtime_websocket::RealtimeSessionMode::Conversational,
        };

        let value = client
            .create(&session, HeaderMap::new())
            .await
            .expect("client secret request should succeed");
        assert_eq!(value, "ek-test-secret");

        let request = transport
            .last_request
            .lock()
            .expect("lock request store")
            .clone()
            .expect("request should be captured");
        assert_eq!(request.method, Method::POST);
        assert_eq!(
            request.url,
            "https://example.com/backend-api/codex/realtime/client_secrets"
        );
        let body = request.body.expect("request body should be present");
        assert_eq!(body["session"]["type"], "realtime");
        assert_eq!(body["session"]["model"], "gpt-realtime");
    }
}
