//! Host-supplied header settings for threads on the Codex Responses endpoint.

use http::HeaderMap;

/// Controls delivery of per-item attribution requested by the Codex backend for
/// accounting. Seed in `StartThreadOptions::thread_extension_init` to set the
/// policy for a thread and its subagents.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UsageAttributionPolicy {
    #[default]
    Exclude,
    Retain,
}

/// Seed in `StartThreadOptions::thread_extension_init` to supply Responses headers.
///
/// Applied only to the selected model on the OpenAI provider with Codex backend
/// auth, regardless of the base URL. The headers are sent on HTTP requests and
/// WebSocket handshakes; changes require a new socket. Other providers and
/// unrelated threads do not inherit these headers.
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
