use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::message_processor::MessageProcessor;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_protocol::protocol::SessionSource;

// We keep the queues bounded so a stalled remote client cannot accumulate
// unbounded JSON output in this process.
const OUTGOING_MESSAGE_BUFFER: usize = 128;
const JSON_MESSAGE_BUFFER: usize = 128;

/// Shared in‑process Codex App Server engine. Holds shared configuration and
/// creates per‑connection processors that can be driven by a JSON‑RPC transport.
#[derive(Clone)]
pub struct AppServerEngine {
    config: Arc<Config>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    auth_manager: Arc<AuthManager>,
    conversation_manager: Arc<ConversationManager>,
}

impl AppServerEngine {
    pub fn new(config: Arc<Config>, codex_linux_sandbox_exe: Option<PathBuf>) -> Self {
        let auth_manager = AuthManager::shared(config.codex_home.clone(), false);
        // Sessions originating from the websocket bridge should be tagged as remote
        // so rollouts and analytics can distinguish them from VS Code or CLI traffic.
        let conversation_manager = Arc::new(ConversationManager::new(
            auth_manager.clone(),
            SessionSource::WSRemote,
        ));
        Self {
            config,
            codex_linux_sandbox_exe,
            auth_manager,
            conversation_manager,
        }
    }

    /// Create a new logical connection. Returns an [`AppServerConnection`] and
    /// a bounded receiver of JSON values that should be forwarded to the client.
    pub fn new_connection(&self) -> (AppServerConnection, mpsc::Receiver<serde_json::Value>) {
        // Outgoing channel used internally by the message processor.
        let (tx_outgoing, mut rx_outgoing) =
            mpsc::channel::<OutgoingMessage>(OUTGOING_MESSAGE_BUFFER);
        let outgoing_sender = OutgoingMessageSender::new(tx_outgoing);

        // Convert internal OutgoingMessage into JSON values for transport.
        let (tx_json, rx_json) = mpsc::channel::<serde_json::Value>(JSON_MESSAGE_BUFFER);
        tokio::spawn(async move {
            let tx_json = tx_json;
            while let Some(msg) = rx_outgoing.recv().await {
                match serde_json::to_value(msg) {
                    Ok(value) => {
                        // Close the task if the client has gone away; this prevents
                        // us from queueing data the transport will never read.
                        if tx_json.send(value).await.is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        // No direct logging here to keep API transport-agnostic.
                        // Dropping the message is acceptable; client cannot recover it anyway.
                        // We still propagate a JSON error once, but bail if the receiver closed.
                        if tx_json
                            .send(serde_json::json!({
                                "error": format!("failed to serialize outgoing message: {err}"),
                            }))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });

        let processor = MessageProcessor::new_with_shared(
            outgoing_sender,
            self.codex_linux_sandbox_exe.clone(),
            self.config.clone(),
            self.auth_manager.clone(),
            self.conversation_manager.clone(),
        );
        let conn = AppServerConnection { processor };
        (conn, rx_json)
    }
}

/// A per‑transport connection that processes JSON‑RPC messages and enqueues
/// outgoing responses/notifications/requests on the provided channel.
pub struct AppServerConnection {
    processor: MessageProcessor,
}

impl AppServerConnection {
    pub async fn process_request(&mut self, request: JSONRPCRequest) {
        self.processor.process_request(request).await;
    }
    pub async fn process_notification(&self, notification: JSONRPCNotification) {
        self.processor.process_notification(notification).await;
    }
    pub async fn process_response(&mut self, response: JSONRPCResponse) {
        self.processor.process_response(response).await;
    }
}
