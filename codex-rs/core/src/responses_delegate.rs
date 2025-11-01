use std::collections::HashMap;
use std::sync::Arc;

use codex_app_server_protocol::ResponsesApiCallParams;
use std::sync::LazyLock;
use tokio::sync::mpsc;

/// Trait implemented by the app-server to delegate `/v1/responses` HTTP over JSON-RPC.
pub trait ResponsesHttpDelegate: Send + Sync {
    /// Start a delegated call. The implementor should send a JSON-RPC request to the client
    /// with the provided params and keep streaming events back via `incoming_event`.
    fn start_call(&self, params: ResponsesApiCallParams);
}

static DELEGATE: LazyLock<std::sync::OnceLock<Arc<dyn ResponsesHttpDelegate>>> =
    LazyLock::new(std::sync::OnceLock::new);

/// Map of call_id -> sender for raw Responses event JSON.
static CALL_EVENT_SENDERS: LazyLock<
    std::sync::Mutex<HashMap<String, mpsc::Sender<serde_json::Value>>>,
> = LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

pub fn register_delegate(delegate: Arc<dyn ResponsesHttpDelegate>) -> Result<(), ()> {
    DELEGATE.get_or_init(|| delegate);
    Ok(())
}

pub fn has_delegate() -> bool {
    DELEGATE.get().is_some()
}

pub fn with_delegate<R>(f: impl FnOnce(&dyn ResponsesHttpDelegate) -> R) -> Option<R> {
    DELEGATE.get().map(|d| f(d.as_ref()))
}

/// Register a sender to receive JSON events for a call_id.
pub fn register_call_channel(call_id: String, tx: mpsc::Sender<serde_json::Value>) {
    if let Ok(mut map) = CALL_EVENT_SENDERS.lock() {
        map.insert(call_id, tx);
    }
}

pub fn finish_call(call_id: &str) {
    if let Ok(mut map) = CALL_EVENT_SENDERS.lock() {
        map.remove(call_id);
    }
}

/// Forward a raw Responses event (JSON envelope) from the app-server client into core.
/// Returns true if an active call was found.
pub fn incoming_event(call_id: &str, event: serde_json::Value) -> bool {
    if let Ok(map) = CALL_EVENT_SENDERS.lock()
        && let Some(tx) = map.get(call_id) {
            let _ = tx.try_send(event);
            return true;
        }
    false
}
