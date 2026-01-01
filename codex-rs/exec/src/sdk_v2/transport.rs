//! Bidirectional stdin/stdout transport for SDK communication.
//!
//! Provides the low-level transport layer for reading/writing JSON-line
//! messages between the CLI and SDK.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use codex_sdk_protocol::control::ControlRequest;
use codex_sdk_protocol::control::ControlRequestEnvelope;
use codex_sdk_protocol::control::ControlResponse;
use codex_sdk_protocol::control::ControlResponseEnvelope;
use codex_sdk_protocol::messages::CliMessage;
use codex_sdk_protocol::messages::SdkMessage;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::sync::Mutex;
use tokio::sync::oneshot;

/// Error type for transport operations.
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Unexpected end of input")]
    EndOfInput,

    #[error("Request timeout")]
    Timeout,

    #[error("Request cancelled")]
    Cancelled,

    #[error("Unexpected message type: expected {expected}, got {actual}")]
    UnexpectedMessage { expected: String, actual: String },
}

/// Pending control request awaiting response.
struct PendingRequest {
    sender: oneshot::Sender<ControlResponse>,
}

/// Bidirectional transport for SDK communication.
pub struct SdkTransport {
    stdin: BufReader<tokio::io::Stdin>,
    stdout: std::io::Stdout,
    pending_requests: Arc<Mutex<HashMap<String, PendingRequest>>>,
}

impl SdkTransport {
    /// Create a new transport using stdin/stdout.
    pub fn new() -> Self {
        Self {
            stdin: BufReader::new(tokio::io::stdin()),
            stdout: std::io::stdout(),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Read the next message from stdin.
    pub async fn read_message(&mut self) -> Result<SdkMessage, TransportError> {
        let mut line = String::new();
        let bytes_read = self.stdin.read_line(&mut line).await?;

        if bytes_read == 0 {
            return Err(TransportError::EndOfInput);
        }

        let message: SdkMessage = serde_json::from_str(line.trim())?;
        Ok(message)
    }

    /// Write a message to stdout.
    pub fn write_message(&mut self, msg: &CliMessage) -> Result<(), TransportError> {
        let json = serde_json::to_string(msg)?;
        writeln!(self.stdout, "{json}")?;
        self.stdout.flush()?;
        Ok(())
    }

    /// Send a control request to the SDK and wait for response.
    ///
    /// This is used when the CLI needs to ask the SDK for a decision
    /// (e.g., permission check, hook callback).
    pub async fn send_control_request(
        &mut self,
        request: ControlRequest,
    ) -> Result<ControlResponse, TransportError> {
        let request_id = uuid::Uuid::new_v4().to_string();

        // Create channel for response
        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(request_id.clone(), PendingRequest { sender: tx });
        }

        // Send the request
        let envelope = ControlRequestEnvelope {
            request_id: request_id.clone(),
            request,
        };
        self.write_message(&CliMessage::ControlRequest(envelope))?;

        // Wait for response
        rx.await.map_err(|_| TransportError::Cancelled)
    }

    /// Handle an incoming control response from the SDK.
    ///
    /// This should be called when a `ControlResponse` message is received
    /// to complete the corresponding pending request.
    pub async fn handle_control_response(
        &self,
        envelope: ControlResponseEnvelope,
    ) -> Result<(), TransportError> {
        let mut pending = self.pending_requests.lock().await;
        if let Some(request) = pending.remove(&envelope.request_id) {
            // Ignore send error - receiver may have been dropped
            let _ = request.sender.send(envelope.response);
        }
        Ok(())
    }

    /// Get a clone of the pending requests map for concurrent access.
    pub fn pending_requests(&self) -> Arc<Mutex<HashMap<String, PendingRequest>>> {
        Arc::clone(&self.pending_requests)
    }
}

impl Default for SdkTransport {
    fn default() -> Self {
        Self::new()
    }
}
