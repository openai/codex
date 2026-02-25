//! HTTP/2 Transport

use super::{TransportConfig, pool::ConnectionPool};

pub struct Http2Transport {
    config: TransportConfig,
    pool: ConnectionPool,
}

impl Http2Transport {
    pub fn new(config: TransportConfig) -> Self {
        let pool = ConnectionPool::new(config.pool_size, config.pool_size / 5);
        Self { config, pool }
    }
    
    pub async fn request(&self, _body: &[u8]) -> Result<Vec<u8>, String> {
        // Placeholder - would use reqwest/hyper with HTTP/2
        Ok(vec![])
    }
}
