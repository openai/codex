use super::*;
use std::sync::Mutex as StdMutex;

#[derive(Default)]
struct RecordingAuthProvider {
    observed_headers: StdMutex<Vec<(HeaderMap, HeaderMap)>>,
}

impl crate::auth::AuthProvider for RecordingAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    fn observe_response_headers(
        &self,
        _request_url: &str,
        request_headers: &HeaderMap,
        response_headers: &HeaderMap,
    ) {
        self.observed_headers
            .lock()
            .expect("recording auth lock should not be poisoned")
            .push((request_headers.clone(), response_headers.clone()));
    }
}

#[test]
fn observe_auth_response_headers_retains_request_and_http_error_headers() {
    let mut request_headers = HeaderMap::new();
    request_headers.insert("x-oai-is", "ois1.sent.nonce.ciphertext".parse().unwrap());
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        "x-oai-is-update",
        "ois1.rotated.nonce.ciphertext".parse().unwrap(),
    );
    let response: Result<Response, TransportError> = Err(TransportError::Http {
        status: http::StatusCode::UNAUTHORIZED,
        url: Some("https://chatgpt.com/backend-api/codex/responses".to_string()),
        headers: Some(response_headers.clone()),
        body: None,
    });
    let auth = RecordingAuthProvider::default();

    observe_auth_response_headers(
        &auth,
        "https://chatgpt.com/backend-api/codex/responses",
        &request_headers,
        &response,
    );

    assert_eq!(
        auth.observed_headers
            .lock()
            .expect("recording auth lock should not be poisoned")
            .as_slice(),
        &[(request_headers, response_headers)]
    );
}
