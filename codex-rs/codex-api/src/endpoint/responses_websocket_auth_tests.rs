use super::*;
use crate::auth::AuthProvider;
use http::HeaderValue;
use http::StatusCode;
use pretty_assertions::assert_eq;
use std::sync::Mutex as StdMutex;

const REQUEST_URL: &str = "wss://chatgpt.com/backend-api/codex/responses";
const SENT_STATE: &str = "sent-state";
const HANDSHAKE_STATE: &str = "handshake-state";
const WRAPPED_ERROR_STATE: &str = "wrapped-error-state";

struct RecordingAuthProvider {
    state: StdMutex<String>,
    observed_updates: StdMutex<Vec<(String, String, String)>>,
}

impl Default for RecordingAuthProvider {
    fn default() -> Self {
        Self {
            state: StdMutex::new(SENT_STATE.to_string()),
            observed_updates: StdMutex::new(Vec::new()),
        }
    }
}

impl AuthProvider for RecordingAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    fn add_auth_headers_for_url(&self, _request_url: &str, headers: &mut HeaderMap) {
        let state = self
            .state
            .lock()
            .expect("state lock should not be poisoned");
        headers.insert(
            "x-test-state",
            HeaderValue::from_str(&state).expect("state should be a valid header value"),
        );
    }

    fn observe_response_headers(
        &self,
        request_url: &str,
        request_headers: &HeaderMap,
        response_headers: &HeaderMap,
    ) {
        let Some(sent_state) = request_headers
            .get("x-test-state")
            .and_then(|value| value.to_str().ok())
        else {
            return;
        };
        let Some(update_state) = response_headers
            .get("x-test-state-update")
            .and_then(|value| value.to_str().ok())
        else {
            return;
        };
        self.observed_updates
            .lock()
            .expect("observed updates lock should not be poisoned")
            .push((
                request_url.to_string(),
                sent_state.to_string(),
                update_state.to_string(),
            ));
        *self
            .state
            .lock()
            .expect("state lock should not be poisoned") = update_state.to_string();
    }
}

#[test]
fn observes_handshake_and_wrapped_error_headers() {
    let auth = Arc::new(RecordingAuthProvider::default());
    let mut request_headers = HeaderMap::new();
    auth.add_auth_headers_for_url(REQUEST_URL, &mut request_headers);
    assert_eq!(
        request_headers
            .get("x-test-state")
            .and_then(|value| value.to_str().ok()),
        Some(SENT_STATE)
    );
    let context = WebsocketAuthContext::new(auth.clone(), REQUEST_URL.to_string(), request_headers);

    let mut handshake_headers = HeaderMap::new();
    handshake_headers.insert(
        "x-test-state-update",
        HeaderValue::from_static(HANDSHAKE_STATE),
    );
    context.observe_response_headers(&handshake_headers);

    let mut wrapped_error_headers = HeaderMap::new();
    wrapped_error_headers.insert(
        "x-test-state-update",
        HeaderValue::from_static(WRAPPED_ERROR_STATE),
    );
    context.observe_error_headers(&ApiError::Transport(TransportError::Http {
        status: StatusCode::BAD_REQUEST,
        url: Some(REQUEST_URL.to_string()),
        headers: Some(wrapped_error_headers),
        body: None,
    }));

    assert_eq!(
        *auth
            .observed_updates
            .lock()
            .expect("observed updates lock should not be poisoned"),
        vec![
            (
                REQUEST_URL.to_string(),
                SENT_STATE.to_string(),
                HANDSHAKE_STATE.to_string(),
            ),
            (
                REQUEST_URL.to_string(),
                HANDSHAKE_STATE.to_string(),
                WRAPPED_ERROR_STATE.to_string(),
            ),
        ]
    );
}
