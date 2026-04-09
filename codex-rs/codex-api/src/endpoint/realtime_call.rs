use crate::auth::AuthProvider;
use crate::endpoint::realtime_websocket::RealtimeSessionConfig;
use crate::endpoint::realtime_websocket::session_update_session_json;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::provider::Provider;
use bytes::Bytes;
use codex_client::HttpTransport;
use codex_client::RequestBody;
use codex_client::RequestTelemetry;
use http::HeaderMap;
use http::HeaderValue;
use http::Method;
use http::header::CONTENT_TYPE;
use http::header::LOCATION;
use serde::Serialize;
use serde_json::Value;
use serde_json::to_string;
use serde_json::to_value;
use std::sync::Arc;
use tracing::instrument;

const MULTIPART_BOUNDARY: &str = "codex-realtime-call-boundary";
const MULTIPART_CONTENT_TYPE: &str = "multipart/form-data; boundary=codex-realtime-call-boundary";

pub struct RealtimeCallClient<T: HttpTransport, A: AuthProvider> {
    session: EndpointSession<T, A>,
}

/// Answer from creating a WebRTC Realtime call.
///
/// `sdp` configures the peer connection. `call_id` is parsed from the response `Location` header
/// and is later used by the server-side sideband WebSocket to join this exact call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealtimeCallResponse {
    pub sdp: String,
    pub call_id: String,
}

#[derive(Serialize)]
struct BackendRealtimeCallRequest<'a> {
    sdp: &'a str,
    session: &'a Value,
}

impl<T: HttpTransport, A: AuthProvider> RealtimeCallClient<T, A> {
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
        "realtime/calls"
    }

    fn uses_backend_request_shape(&self) -> bool {
        self.session.provider().base_url.contains("/backend-api")
    }

    #[instrument(
        name = "realtime_call.create",
        level = "info",
        skip_all,
        fields(
            http.method = "POST",
            api.path = "realtime/calls"
        )
    )]
    pub async fn create(&self, sdp: String) -> Result<RealtimeCallResponse, ApiError> {
        self.create_with_headers(sdp, HeaderMap::new()).await
    }

    pub async fn create_with_session(
        &self,
        sdp: String,
        session_config: RealtimeSessionConfig,
    ) -> Result<RealtimeCallResponse, ApiError> {
        self.create_with_session_and_headers(sdp, session_config, HeaderMap::new())
            .await
    }

    pub async fn create_with_headers(
        &self,
        sdp: String,
        extra_headers: HeaderMap,
    ) -> Result<RealtimeCallResponse, ApiError> {
        let resp = self
            .session
            .execute_with(
                Method::POST,
                Self::path(),
                extra_headers,
                /*body*/ None,
                |req| {
                    req.headers
                        .insert(CONTENT_TYPE, HeaderValue::from_static("application/sdp"));
                    req.body = Some(RequestBody::Raw(Bytes::from(sdp.clone())));
                },
            )
            .await?;

        let sdp = decode_sdp_response(resp.body.as_ref())?;
        let call_id = decode_call_id_from_location(&resp.headers)?;

        Ok(RealtimeCallResponse { sdp, call_id })
    }

    pub async fn create_with_session_and_headers(
        &self,
        sdp: String,
        session_config: RealtimeSessionConfig,
        extra_headers: HeaderMap,
    ) -> Result<RealtimeCallResponse, ApiError> {
        // WebRTC can begin inference as soon as the peer connection comes up, so the initial
        // session payload is sent with call creation. The sideband WebSocket still sends its normal
        // session.update after it joins.
        let mut session = realtime_session_json(session_config)?;
        if let Some(session) = session.as_object_mut() {
            session.remove("id");
        }
        // TODO(aibrahim): Align the SIWC route with the API multipart shape and remove this branch.
        if self.uses_backend_request_shape() {
            let body = to_value(BackendRealtimeCallRequest {
                sdp: &sdp,
                session: &session,
            })
            .map_err(|err| ApiError::Stream(format!("failed to encode realtime call: {err}")))?;
            let resp = self
                .session
                .execute(Method::POST, Self::path(), extra_headers, Some(body))
                .await?;
            let sdp = decode_sdp_response(resp.body.as_ref())?;
            let call_id = decode_call_id_from_location(&resp.headers)?;
            return Ok(RealtimeCallResponse { sdp, call_id });
        }

        let session = to_string(&session).map_err(|err| ApiError::InvalidRequest {
            message: err.to_string(),
        })?;
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{MULTIPART_BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"sdp\"\r\n");
        body.extend_from_slice(b"Content-Type: application/sdp\r\n\r\n");
        body.extend_from_slice(sdp.as_bytes());
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{MULTIPART_BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"session\"\r\n");
        body.extend_from_slice(b"Content-Type: application/json\r\n\r\n");
        body.extend_from_slice(session.as_bytes());
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{MULTIPART_BOUNDARY}--\r\n").as_bytes());

        let resp = self
            .session
            .execute_with(
                Method::POST,
                Self::path(),
                extra_headers,
                /*body*/ None,
                |req| {
                    req.headers.insert(
                        CONTENT_TYPE,
                        HeaderValue::from_static(MULTIPART_CONTENT_TYPE),
                    );
                    req.body = Some(RequestBody::Raw(Bytes::from(body.clone())));
                },
            )
            .await?;

        let sdp = decode_sdp_response(resp.body.as_ref())?;
        let call_id = decode_call_id_from_location(&resp.headers)?;

        Ok(RealtimeCallResponse { sdp, call_id })
    }
}

fn realtime_session_json(session_config: RealtimeSessionConfig) -> Result<Value, ApiError> {
    session_update_session_json(session_config)
        .map_err(|err| ApiError::Stream(format!("failed to encode realtime call session: {err}")))
}

fn decode_sdp_response(body: &[u8]) -> Result<String, ApiError> {
    String::from_utf8(body.to_vec()).map_err(|err| {
        ApiError::Stream(format!(
            "failed to decode realtime call SDP response: {err}"
        ))
    })
}

fn decode_call_id_from_location(headers: &HeaderMap) -> Result<String, ApiError> {
    let location = headers
        .get(LOCATION)
        .ok_or_else(|| ApiError::Stream("realtime call response missing Location".to_string()))?
        .to_str()
        .map_err(|err| ApiError::Stream(format!("invalid realtime call Location: {err}")))?;

    location
        .split('?')
        .next()
        .unwrap_or(location)
        .rsplit('/')
        .find(|segment| segment.starts_with("rtc_") && segment.len() > "rtc_".len())
        .map(str::to_string)
        .ok_or_else(|| {
            ApiError::Stream(format!(
                "realtime call Location does not contain a call id: {location}"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::endpoint::realtime_websocket::RealtimeEventParser;
    use crate::endpoint::realtime_websocket::RealtimeSessionMode;
    use crate::provider::RetryConfig;
    use async_trait::async_trait;
    use codex_client::Request;
    use codex_client::Response;
    use codex_client::StreamResponse;
    use codex_client::TransportError;
    use codex_protocol::protocol::RealtimeVoice;
    use http::StatusCode;
    use pretty_assertions::assert_eq;
    use std::sync::Mutex;
    use std::time::Duration;

    #[derive(Clone)]
    struct CapturingTransport {
        last_request: Arc<Mutex<Option<Request>>>,
        response_headers: HeaderMap,
    }

    impl CapturingTransport {
        fn new() -> Self {
            Self::with_location("/v1/realtime/calls/rtc_test")
        }

        fn with_location(location: &str) -> Self {
            let mut response_headers = HeaderMap::new();
            response_headers.insert(LOCATION, HeaderValue::from_str(location).unwrap());
            Self {
                last_request: Arc::new(Mutex::new(None)),
                response_headers,
            }
        }

        fn without_location() -> Self {
            Self {
                last_request: Arc::new(Mutex::new(None)),
                response_headers: HeaderMap::new(),
            }
        }
    }

    #[async_trait]
    impl HttpTransport for CapturingTransport {
        async fn execute(&self, req: Request) -> Result<Response, TransportError> {
            *self.last_request.lock().unwrap() = Some(req);
            Ok(Response {
                status: StatusCode::OK,
                headers: self.response_headers.clone(),
                body: Bytes::from_static(b"v=0\r\n"),
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
            Some("test-token".to_string())
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

    fn realtime_session_config(session_id: &str) -> RealtimeSessionConfig {
        RealtimeSessionConfig {
            instructions: "hi".to_string(),
            model: Some("gpt-realtime".to_string()),
            session_id: Some(session_id.to_string()),
            event_parser: RealtimeEventParser::RealtimeV2,
            session_mode: RealtimeSessionMode::Conversational,
            voice: RealtimeVoice::Marin,
        }
    }

    #[tokio::test]
    async fn sends_sdp_offer_as_raw_body() {
        let transport = CapturingTransport::new();
        let client = RealtimeCallClient::new(
            transport.clone(),
            provider("https://api.openai.com/v1"),
            DummyAuth,
        );

        let response = client
            .create("v=offer\r\n".to_string())
            .await
            .expect("request should succeed");

        assert_eq!(
            response,
            RealtimeCallResponse {
                sdp: "v=0\r\n".to_string(),
                call_id: "rtc_test".to_string(),
            }
        );

        let request = transport.last_request.lock().unwrap().clone().unwrap();
        assert_eq!(request.method, Method::POST);
        assert_eq!(request.url, "https://api.openai.com/v1/realtime/calls");
        assert_eq!(
            request.headers.get(CONTENT_TYPE).unwrap(),
            HeaderValue::from_static("application/sdp")
        );
        assert_eq!(
            request
                .headers
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer test-token")
        );
        assert_eq!(
            request.body,
            Some(RequestBody::Raw(Bytes::from_static(b"v=offer\r\n")))
        );
    }

    #[tokio::test]
    async fn extracts_call_id_from_forwarded_backend_location() {
        let transport =
            CapturingTransport::with_location("/v1/realtime/calls/calls/rtc_backend_test");
        let client = RealtimeCallClient::new(
            transport.clone(),
            provider("https://chatgpt.com/backend-api/codex"),
            DummyAuth,
        );

        let response = client
            .create("v=offer\r\n".to_string())
            .await
            .expect("request should succeed");

        assert_eq!(
            response,
            RealtimeCallResponse {
                sdp: "v=0\r\n".to_string(),
                call_id: "rtc_backend_test".to_string(),
            }
        );

        let request = transport.last_request.lock().unwrap().clone().unwrap();
        assert_eq!(request.method, Method::POST);
        assert_eq!(
            request.url,
            "https://chatgpt.com/backend-api/codex/realtime/calls"
        );
        assert_eq!(
            request.body,
            Some(RequestBody::Raw(Bytes::from_static(b"v=offer\r\n")))
        );
    }

    #[tokio::test]
    async fn sends_api_session_call_as_multipart_body() {
        let transport = CapturingTransport::new();
        let client = RealtimeCallClient::new(
            transport.clone(),
            provider("https://api.openai.com/v1"),
            DummyAuth,
        );

        let response = client
            .create_with_session(
                "v=offer\r\n".to_string(),
                realtime_session_config("sess-api"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(
            response,
            RealtimeCallResponse {
                sdp: "v=0\r\n".to_string(),
                call_id: "rtc_test".to_string(),
            }
        );

        let request = transport.last_request.lock().unwrap().clone().unwrap();
        assert_eq!(request.method, Method::POST);
        assert_eq!(request.url, "https://api.openai.com/v1/realtime/calls");
        assert_eq!(
            request.headers.get(CONTENT_TYPE).unwrap(),
            HeaderValue::from_static(MULTIPART_CONTENT_TYPE)
        );
        let Some(RequestBody::Raw(body)) = request.body else {
            panic!("multipart body should be raw");
        };
        let body = std::str::from_utf8(&body).expect("multipart body should be utf-8");
        let mut session = realtime_session_json(realtime_session_config("sess-api"))
            .expect("session should encode");
        session
            .as_object_mut()
            .expect("session should be an object")
            .remove("id");
        let session = to_string(&session).expect("session should serialize");
        assert_eq!(
            body,
            format!(
                "--codex-realtime-call-boundary\r\n\
                 Content-Disposition: form-data; name=\"sdp\"\r\n\
                 Content-Type: application/sdp\r\n\
                 \r\n\
                 v=offer\r\n\
                 \r\n\
                 --codex-realtime-call-boundary\r\n\
                 Content-Disposition: form-data; name=\"session\"\r\n\
                 Content-Type: application/json\r\n\
                 \r\n\
                 {session}\r\n\
                 --codex-realtime-call-boundary--\r\n"
            )
        );
    }

    #[tokio::test]
    async fn sends_backend_session_call_as_json_body() {
        let transport = CapturingTransport::new();
        let client = RealtimeCallClient::new(
            transport.clone(),
            provider("https://chatgpt.com/backend-api/codex"),
            DummyAuth,
        );

        let response = client
            .create_with_session(
                "v=offer\r\n".to_string(),
                realtime_session_config("sess-backend"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(
            response,
            RealtimeCallResponse {
                sdp: "v=0\r\n".to_string(),
                call_id: "rtc_test".to_string(),
            }
        );

        let request = transport.last_request.lock().unwrap().clone().unwrap();
        assert_eq!(request.method, Method::POST);
        assert_eq!(
            request.url,
            "https://chatgpt.com/backend-api/codex/realtime/calls"
        );
        let mut expected_session = realtime_session_json(realtime_session_config("sess-backend"))
            .expect("session should encode");
        expected_session
            .as_object_mut()
            .expect("session should be an object")
            .remove("id");
        assert_eq!(
            request.body,
            Some(RequestBody::Json(
                to_value(BackendRealtimeCallRequest {
                    sdp: "v=offer\r\n",
                    session: &expected_session,
                })
                .expect("request should encode")
            ))
        );
    }

    #[tokio::test]
    async fn errors_when_location_is_missing() {
        let transport = CapturingTransport::without_location();
        let client =
            RealtimeCallClient::new(transport, provider("https://api.openai.com/v1"), DummyAuth);

        let err = client
            .create("v=offer\r\n".to_string())
            .await
            .expect_err("request should require Location");

        assert_eq!(
            err.to_string(),
            "stream error: realtime call response missing Location"
        );
    }

    #[test]
    fn rejects_location_without_call_id() {
        let mut headers = HeaderMap::new();
        headers.insert(LOCATION, HeaderValue::from_static("/v1/realtime/calls"));

        let err = decode_call_id_from_location(&headers)
            .expect_err("Location without rtc_ segment should fail");

        assert_eq!(
            err.to_string(),
            "stream error: realtime call Location does not contain a call id: /v1/realtime/calls"
        );
    }
}
