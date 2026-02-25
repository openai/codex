//! Unix Domain Socket Transport for local IPC

use super::TransportConfig;
use std::path::Path;

pub struct UnixSocketTransport {
    config: TransportConfig,
}

impl UnixSocketTransport {
    pub fn new(config: TransportConfig) -> Self {
        Self { config }
    }
    
    pub fn is_available(&self) -> bool {
        Path::new(&self.config.unix_socket_path).exists()
    }
    
    pub async fn request(&self, _body: &[u8]) -> Result<Vec<u8>, String> {
        // Placeholder - would use tokio::net::UnixStream
        Ok(vec![])
    }
}
