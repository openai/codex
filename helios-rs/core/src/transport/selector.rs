//! Transport Selector - Auto-selects optimal transport

use super::{TransportConfig, TransportType};
use std::path::Path;

/// Auto-selects the best transport based on environment
pub struct TransportSelector {
    config: TransportConfig,
}

impl TransportSelector {
    pub fn new(config: TransportConfig) -> Self {
        Self { config }
    }
    
    /// Select the best transport
    pub fn select(&self) -> TransportType {
        // Priority: Unix Socket > WebSocket > HTTP/2
        if self.is_unix_socket_available() {
            TransportType::UnixSocket
        } else if self.is_websocket_available() {
            TransportType::WebSocket
        } else {
            TransportType::Http2
        }
    }
    
    fn is_unix_socket_available(&self) -> bool {
        Path::new(&self.config.unix_socket_path).exists()
    }
    
    fn is_websocket_available(&self) -> bool {
        // WebSocket is always "available" if configured
        !self.config.ws_url.is_empty()
    }
}

impl Default for TransportSelector {
    fn default() -> Self {
        Self::new(TransportConfig::default())
    }
}
