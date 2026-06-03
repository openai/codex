use super::*;
use crate::provider::RetryConfig;
use async_trait::async_trait;
use codex_client::Request;
use http::HeaderValue;
use http::StatusCode;
use pretty_assertions::assert_eq;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

const RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";

#[derive(Default)]
struct RecordingAuthProvider {
    observed_headers: StdMutex<Vec<(String, HeaderMap, HeaderMap)>>,
}

impl crate::auth::AuthProvider for RecordingAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    fn add_auth_headers_for_url(&self, _request_url: &str, headers: &mut HeaderMap) {
        headers.insert("x-test-state", HeaderValue::from_static("sent-state"));
    }

    fn observe_response_headers(
        &self,
        request_url: &str,
        request_headers: &HeaderMap,
        response_headers: &HeaderMap,
    ) {
        self.observed_headers
            .lock()
            .expect("recording auth lock should not be poisoned")
            .push((
                request_url.to_string(),
                request_headers.clone(),
                response_headers.clone(),
            ));
    }
}

#[derive(Clone)]
struct RejectingTransport {
    requests: Arc<StdMutex<Vec<(String, HeaderMap)>>>,
    response_headers: HeaderMap,
}

impl RejectingTransport {
    fn new(response_headers: HeaderMap) -> Self {
        Self {
            requests: Arc::new(StdMutex::new(Vec::new())),
            response_headers,
        }
    }

    fn http_error(&self, request: Request) -> TransportError {
        self.requests
            .lock()
            .expect("recording transport lock should not be poisoned")
            .push((request.url.clone(), request.headers));
        TransportError::Http {
            status: StatusCode::UNAUTHORIZED,
            url: Some(request.url),
            headers: Some(self.response_headers.clone()),
            body: None,
        }
    }
}

#[async_trait]
impl HttpTransport for RejectingTransport {
    async fn execute(&self, request: Request) -> Result<Response, TransportError> {
        Err(self.http_error(request))
    }

    async fn stream(&self, request: Request) -> Result<StreamResponse, TransportError> {
        Err(self.http_error(request))
    }
}

#[tokio::test]
async fn execute_attaches_url_scoped_auth_and_observes_http_error_headers() {
    let (session, transport, auth, request_headers, response_headers) = test_session();

    session
        .execute(
            Method::POST,
            "responses",
            HeaderMap::new(),
            /*body*/ None,
        )
        .await
        .expect_err("request should fail");

    assert_request_and_response_headers(&transport, &auth, request_headers, response_headers);
}

#[tokio::test]
async fn stream_attaches_url_scoped_auth_and_observes_http_error_headers() {
    let (session, transport, auth, request_headers, response_headers) = test_session();

    let result = session
        .stream_with(
            Method::POST,
            "responses",
            HeaderMap::new(),
            /*body*/ None,
            |_| {},
        )
        .await;
    assert!(result.is_err(), "request should fail");

    assert_request_and_response_headers(&transport, &auth, request_headers, response_headers);
}

fn test_session() -> (
    EndpointSession<RejectingTransport>,
    RejectingTransport,
    Arc<RecordingAuthProvider>,
    HeaderMap,
    HeaderMap,
) {
    let mut request_headers = HeaderMap::new();
    request_headers.insert("x-test-state", HeaderValue::from_static("sent-state"));
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        "x-test-state-update",
        HeaderValue::from_static("rotated-state"),
    );
    let transport = RejectingTransport::new(response_headers.clone());
    let auth = Arc::new(RecordingAuthProvider::default());
    let session = EndpointSession::new(transport.clone(), provider(), auth.clone());
    (session, transport, auth, request_headers, response_headers)
}

fn provider() -> Provider {
    Provider {
        name: "test".to_string(),
        base_url: "https://chatgpt.com/backend-api/codex".to_string(),
        query_params: None,
        headers: HeaderMap::new(),
        retry: RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(1),
            retry_429: false,
            retry_5xx: false,
            retry_transport: false,
        },
        stream_idle_timeout: Duration::from_secs(1),
    }
}

fn assert_request_and_response_headers(
    transport: &RejectingTransport,
    auth: &RecordingAuthProvider,
    request_headers: HeaderMap,
    response_headers: HeaderMap,
) {
    assert_eq!(
        transport
            .requests
            .lock()
            .expect("recording transport lock should not be poisoned")
            .as_slice(),
        &[(RESPONSES_URL.to_string(), request_headers.clone())]
    );
    assert_eq!(
        auth.observed_headers
            .lock()
            .expect("recording auth lock should not be poisoned")
            .as_slice(),
        &[(RESPONSES_URL.to_string(), request_headers, response_headers,)]
    );
}
