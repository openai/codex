use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

#[derive(Clone, Debug, Default)]
pub struct TransportManager {
    disable_websockets: Arc<AtomicBool>,
}

impl TransportManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn disable_websockets(&self) -> bool {
        self.disable_websockets.load(Ordering::Relaxed)
    }

    pub fn activate_http_fallback(&self, websocket_enabled: bool) -> bool {
        websocket_enabled && !self.disable_websockets.swap(true, Ordering::Relaxed)
    }
}
