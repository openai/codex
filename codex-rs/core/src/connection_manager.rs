use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::model_provider_info::WireApi;

/// Session-scoped connection transport state.
///
/// Once fallback is activated, the session will stick to HTTP for the rest of
/// its lifetime when the provider prefers the WebSocket transport.
#[derive(Clone, Debug, Default)]
pub struct TransportManager {
    fallback_to_http: Arc<AtomicBool>,
}

impl TransportManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn effective_wire_api(&self, provider_wire_api: WireApi) -> WireApi {
        if self.fallback_to_http.load(Ordering::Relaxed)
            && provider_wire_api == WireApi::ResponsesWebsocket
        {
            WireApi::Responses
        } else {
            provider_wire_api
        }
    }

    /// Activates sticky HTTP fallback. Returns `true` if this call flipped the
    /// state from WebSocket to HTTP.
    pub fn activate_http_fallback(&self, provider_wire_api: WireApi) -> bool {
        provider_wire_api == WireApi::ResponsesWebsocket
            && !self.fallback_to_http.swap(true, Ordering::Relaxed)
    }
}
