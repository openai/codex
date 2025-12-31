//! JSON-RPC 2.0 over stdio implementation

use crate::config::LifecycleConfig;
use crate::error::LspErr;
use crate::error::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio::time::timeout;
use tracing::debug;
use tracing::info;
use tracing::trace;
use tracing::warn;

/// Default request timeout in seconds (legacy, use TimeoutConfig instead)
pub const REQUEST_TIMEOUT_SECS: i32 = 30;

/// Initialization timeout in seconds (legacy, use TimeoutConfig instead)
pub const INIT_TIMEOUT_SECS: i32 = 45;

/// Maximum allowed Content-Length for LSP messages (10 MB)
/// Prevents memory exhaustion from malformed or malicious servers
const MAX_CONTENT_LENGTH: usize = 10 * 1024 * 1024;

/// Configurable timeout settings for LSP operations
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Initialization timeout in milliseconds
    pub init_timeout_ms: i64,
    /// Request timeout in milliseconds
    pub request_timeout_ms: i64,
    /// Shutdown timeout in milliseconds
    pub shutdown_timeout_ms: i64,
    /// Notification channel buffer size
    pub notification_buffer_size: i32,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            init_timeout_ms: 10_000,
            request_timeout_ms: 30_000,
            shutdown_timeout_ms: 5_000,
            notification_buffer_size: 100,
        }
    }
}

impl From<&LifecycleConfig> for TimeoutConfig {
    fn from(config: &LifecycleConfig) -> Self {
        Self {
            init_timeout_ms: config.startup_timeout_ms,
            request_timeout_ms: config.request_timeout_ms,
            shutdown_timeout_ms: config.shutdown_timeout_ms,
            notification_buffer_size: config.notification_buffer_size,
        }
    }
}

impl TimeoutConfig {
    /// Get init timeout as Duration
    pub fn init_timeout(&self) -> Duration {
        Duration::from_millis(self.init_timeout_ms as u64)
    }

    /// Get request timeout as Duration
    pub fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.request_timeout_ms as u64)
    }

    /// Get shutdown timeout as Duration
    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_millis(self.shutdown_timeout_ms as u64)
    }

    /// Get init timeout in seconds (for legacy API compatibility)
    pub fn init_timeout_secs(&self) -> i32 {
        (self.init_timeout_ms / 1000) as i32
    }

    /// Get request timeout in seconds (for legacy API compatibility)
    pub fn request_timeout_secs(&self) -> i32 {
        (self.request_timeout_ms / 1000) as i32
    }
}

type RequestId = i64;

#[derive(Debug, Serialize)]
struct JsonRpcRequest<T: Serialize> {
    jsonrpc: &'static str,
    id: RequestId,
    method: String,
    params: T,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<RequestId>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    /// Optional additional data per JSON-RPC 2.0 spec
    #[allow(dead_code)]
    data: Option<serde_json::Value>,
}

/// Pending request handle
struct PendingRequest {
    tx: oneshot::Sender<Result<serde_json::Value>>,
    method: String,
}

/// JSON-RPC connection over stdio
pub struct JsonRpcConnection {
    next_id: AtomicI64,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<RequestId, PendingRequest>>>,
    /// Shutdown signal sender
    shutdown_tx: watch::Sender<bool>,
    /// Reader task handle for cleanup
    reader_handle: Mutex<Option<JoinHandle<()>>>,
}

impl JsonRpcConnection {
    /// Create connection from child process stdio
    pub fn new(
        stdin: ChildStdin,
        stdout: ChildStdout,
        notification_tx: mpsc::Sender<(String, serde_json::Value)>,
    ) -> Self {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = Arc::clone(&pending);

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Spawn reader task with shutdown support
        let reader_handle = tokio::spawn(async move {
            if let Err(e) =
                Self::read_loop_with_shutdown(stdout, pending_clone, notification_tx, shutdown_rx)
                    .await
            {
                warn!("LSP read loop ended with error: {}", e);
            }
        });

        info!("JSON-RPC connection established");

        Self {
            next_id: AtomicI64::new(1),
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
            shutdown_tx,
            reader_handle: Mutex::new(Some(reader_handle)),
        }
    }

    /// Send request and await response
    pub async fn request<P: Serialize>(
        &self,
        method: &str,
        params: P,
    ) -> Result<serde_json::Value> {
        self.request_with_timeout(method, params, REQUEST_TIMEOUT_SECS)
            .await
    }

    /// Send request with custom timeout
    pub async fn request_with_timeout<P: Serialize>(
        &self,
        method: &str,
        params: P,
        timeout_secs: i32,
    ) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending.lock().await;
            pending.insert(
                id,
                PendingRequest {
                    tx,
                    method: method.to_string(),
                },
            );
        }

        // Serialize and send
        let body = serde_json::to_string(&request)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        debug!("LSP request [{}]: {}", id, method);
        trace!("LSP request [{}]: {} {}", id, method, body);

        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(message.as_bytes()).await?;
            stdin.flush().await?;
        }

        // Wait for response with timeout
        let method_clone = method.to_string();
        match timeout(Duration::from_secs(timeout_secs as u64), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LspErr::Internal("request cancelled".to_string())),
            Err(_) => {
                // Remove pending request on timeout
                {
                    let mut pending = self.pending.lock().await;
                    pending.remove(&id);
                }

                // Send cancel notification to server (best effort)
                self.cancel_request(id).await;

                warn!(
                    "LSP request [{}] ({}) timed out after {}s - cancel sent",
                    id, method_clone, timeout_secs
                );
                Err(LspErr::RequestTimeout { timeout_secs })
            }
        }
    }

    /// Cancel a pending request
    ///
    /// Sends $/cancelRequest notification to the server.
    /// This is a best-effort operation - the server may not support cancellation.
    pub async fn cancel_request(&self, id: RequestId) {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "$/cancelRequest",
            "params": { "id": id }
        });

        if let Ok(body) = serde_json::to_string(&notification) {
            let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
            let mut stdin = self.stdin.lock().await;
            let _ = stdin.write_all(message.as_bytes()).await;
            let _ = stdin.flush().await;
            debug!("Sent $/cancelRequest for request {}", id);
        }
    }

    /// Send notification (no response expected)
    pub async fn notify<P: Serialize>(&self, method: &str, params: P) -> Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        let body = serde_json::to_string(&notification)?;
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        debug!("LSP notify: {}", method);
        trace!("LSP notify: {} {}", method, body);

        let mut stdin = self.stdin.lock().await;
        stdin.write_all(message.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Read loop for incoming messages (legacy - without shutdown support)
    #[allow(dead_code)]
    async fn read_loop(
        stdout: ChildStdout,
        pending: Arc<Mutex<HashMap<RequestId, PendingRequest>>>,
        notification_tx: mpsc::Sender<(String, serde_json::Value)>,
    ) -> Result<()> {
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        Self::read_loop_with_shutdown(stdout, pending, notification_tx, shutdown_rx).await
    }

    /// Read loop for incoming messages with shutdown support
    async fn read_loop_with_shutdown(
        stdout: ChildStdout,
        pending: Arc<Mutex<HashMap<RequestId, PendingRequest>>>,
        notification_tx: mpsc::Sender<(String, serde_json::Value)>,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<()> {
        let mut reader = BufReader::new(stdout);

        loop {
            // Check shutdown signal
            if *shutdown_rx.borrow() {
                debug!("LSP read loop received shutdown signal");
                // Fail all pending requests
                let mut pending_guard = pending.lock().await;
                for (_id, req) in pending_guard.drain() {
                    let _ = req.tx.send(Err(LspErr::ConnectionClosed));
                }
                return Ok(());
            }

            // Read headers until empty line
            let mut content_length: Option<usize> = None;

            loop {
                let mut header = String::new();

                // Use select to check for shutdown while reading
                let bytes_read = tokio::select! {
                    result = reader.read_line(&mut header) => result?,
                    _ = shutdown_rx.changed() => {
                        debug!("LSP read loop shutdown during header read");
                        let mut pending_guard = pending.lock().await;
                        for (_id, req) in pending_guard.drain() {
                            let _ = req.tx.send(Err(LspErr::ConnectionClosed));
                        }
                        return Ok(());
                    }
                };

                if bytes_read == 0 {
                    let pending_guard = pending.lock().await;
                    let pending_count = pending_guard.len();
                    drop(pending_guard);

                    info!(
                        "LSP connection closed (pending requests: {})",
                        pending_count
                    );

                    // Fail all pending requests so they don't hang
                    let mut pending_guard = pending.lock().await;
                    for (_id, req) in pending_guard.drain() {
                        let _ = req.tx.send(Err(LspErr::ConnectionClosed));
                    }
                    return Ok(());
                }

                let header = header.trim();
                if header.is_empty() {
                    break;
                }

                if let Some(len_str) = header.strip_prefix("Content-Length:") {
                    if let Ok(len) = len_str.trim().parse::<usize>() {
                        content_length = Some(len);
                    }
                }
            }

            let content_length = match content_length {
                Some(len) => len,
                None => {
                    warn!("Missing Content-Length header");
                    continue;
                }
            };

            // Validate Content-Length to prevent memory exhaustion
            if content_length > MAX_CONTENT_LENGTH {
                warn!(
                    "Content-Length {} exceeds maximum allowed {} bytes, skipping message",
                    content_length, MAX_CONTENT_LENGTH
                );
                continue;
            }

            // Read body
            let mut buffer = vec![0u8; content_length];
            reader.read_exact(&mut buffer).await?;

            // Parse message with strict UTF-8 validation
            let raw = match String::from_utf8(buffer) {
                Ok(s) => s,
                Err(e) => {
                    warn!("Invalid UTF-8 in LSP message: {}", e);
                    continue;
                }
            };
            trace!("LSP received: {}", raw);

            let value: serde_json::Value = match serde_json::from_str(&raw) {
                Ok(v) => v,
                Err(e) => {
                    warn!("Failed to parse LSP message: {}", e);
                    continue;
                }
            };

            // Check if response or notification
            if value.get("id").is_some() {
                // Response
                if let Ok(response) = serde_json::from_value::<JsonRpcResponse>(value) {
                    if let Some(id) = response.id {
                        let mut pending_guard = pending.lock().await;
                        if let Some(req) = pending_guard.remove(&id) {
                            let result = if let Some(err) = response.error {
                                Err(LspErr::JsonRpc {
                                    method: req.method.clone(),
                                    message: err.message,
                                    code: Some(err.code),
                                })
                            } else {
                                Ok(response.result.unwrap_or(serde_json::Value::Null))
                            };
                            let _ = req.tx.send(result);
                        }
                    }
                }
            } else if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
                // Notification - check for backpressure
                let params = value
                    .get("params")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let notification = (method.to_string(), params);

                // Try non-blocking send first to detect backpressure
                match notification_tx.try_send(notification) {
                    Ok(()) => {}
                    Err(tokio::sync::mpsc::error::TrySendError::Full(notification)) => {
                        // Channel is full - log warning and block
                        warn!(
                            "LSP notification channel full, backpressure detected (method: {})",
                            notification.0
                        );
                        // Fall back to blocking send
                        let _ = notification_tx.send(notification).await;
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                        // Channel closed, reader is shutting down
                        debug!("Notification channel closed");
                        return Ok(());
                    }
                }
            }
        }
    }
}

impl Drop for JsonRpcConnection {
    fn drop(&mut self) {
        // Signal shutdown to reader task
        let _ = self.shutdown_tx.send(true);

        // Abort reader task if still running
        if let Some(handle) = self.reader_handle.get_mut().take() {
            handle.abort();
            debug!("JsonRpcConnection dropped - reader task aborted");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "test".to_string(),
            params: serde_json::json!({"key": "value"}),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"test\""));
    }

    #[test]
    fn test_json_rpc_response_parsing() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"data":"test"}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, Some(1));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_json_rpc_error_parsing() {
        let json =
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, Some(1));
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        let err = response.error.unwrap();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
    }
}
