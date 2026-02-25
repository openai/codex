//! Connection Pool for HTTP/2 multiplexing

use std::sync::Arc;
use std::collections::VecDeque;
use tokio::sync::Mutex;

/// Pooled connection wrapper
pub struct PooledConnection {
    pub id: u64,
    pub created_at: std::time::Instant,
    pub last_used: std::time::Instant,
}

/// Connection pool manager
pub struct ConnectionPool {
    max_connections: usize,
    keepalive: usize,
    connections: Arc<Mutex<VecDeque<PooledConnection>>>,
    next_id: Arc<Mutex<u64>>,
}

impl ConnectionPool {
    pub fn new(max_connections: usize, keepalive: usize) -> Self {
        Self {
            max_connections,
            keepalive,
            connections: Arc::new(Mutex::new(VecDeque::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }
    
    pub async fn acquire(&self) -> PooledConnection {
        let mut connections = self.connections.lock().await;
        
        // Try to reuse existing connection
        if let Some(conn) = connections.pop_front() {
            return conn;
        }
        
        // Create new connection
        let mut next_id = self.next_id.lock().await;
        let id = *next_id;
        *next_id += 1;
        
        PooledConnection {
            id,
            created_at: std::time::Instant::now(),
            last_used: std::time::Instant::now(),
        }
    }
    
    pub async fn release(&self, conn: PooledConnection) {
        let mut connections = self.connections.lock().await;
        if connections.len() < self.keepalive {
            connections.push_back(conn);
        }
    }
    
    pub async fn size(&self) -> usize {
        self.connections.lock().await.len()
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new(100, 20)
    }
}
