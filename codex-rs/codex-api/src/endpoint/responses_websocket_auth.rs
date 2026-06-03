use crate::auth::SharedAuthProvider;
use crate::error::ApiError;
use codex_client::TransportError;
use http::HeaderMap;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone)]
pub(super) struct WebsocketAuthContext {
    auth: SharedAuthProvider,
    request_url: String,
    request_headers: Arc<Mutex<HeaderMap>>,
}

impl WebsocketAuthContext {
    pub(super) fn new(
        auth: SharedAuthProvider,
        request_url: String,
        request_headers: HeaderMap,
    ) -> Self {
        Self {
            auth,
            request_url,
            request_headers: Arc::new(Mutex::new(request_headers)),
        }
    }

    pub(super) fn observe_response_headers(&self, response_headers: &HeaderMap) {
        let mut request_headers = self
            .request_headers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.auth
            .observe_response_headers(&self.request_url, &request_headers, response_headers);
        self.auth
            .add_auth_headers_for_url(&self.request_url, &mut request_headers);
    }

    pub(super) fn observe_error_headers(&self, error: &ApiError) {
        if let ApiError::Transport(TransportError::Http {
            headers: Some(response_headers),
            ..
        }) = error
        {
            self.observe_response_headers(response_headers);
        }
    }
}
