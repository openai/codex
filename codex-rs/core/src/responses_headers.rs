//! Header overrides for host-created threads on the Codex Responses endpoint.
//! Headers are scoped to a model and rechecked against current auth on each attempt.

use http::HeaderMap;

/// Seed in `StartThreadOptions::thread_extension_init` to supply Responses headers.
///
/// Applied only to the selected model with Codex backend auth and routing. The headers
/// are sent on HTTP requests and WebSocket handshakes; changes require a new socket.
/// Other provider endpoints and unrelated threads do not inherit these headers.
#[derive(Clone)]
pub struct CodexResponsesHeaders {
    pub model: String,
    pub headers: HeaderMap,
}

impl std::fmt::Debug for CodexResponsesHeaders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodexResponsesHeaders")
            .field("model", &self.model)
            .field("header_names", &self.headers.keys())
            .finish()
    }
}
