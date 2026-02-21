use std::path::PathBuf;
use std::sync::Arc;

use crate::codex_message_processor::CodexMessageProcessor;
use crate::codex_message_processor::CodexMessageProcessorArgs;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::OutgoingMessageSender;
use crate::transport::CHANNEL_CAPACITY;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use codex_cloud_requirements::cloud_requirements_loader;
use codex_core::AuthManager;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_feedback::CodexFeedback;
use tokio::sync::mpsc;
use toml::Value as TomlValue;

const EMBEDDED_CONNECTION_ID: ConnectionId = ConnectionId(0);

#[derive(Debug)]
enum EmbeddedSessionInput {
    Request(ClientRequest),
    Response(JSONRPCResponse),
    Error(JSONRPCError),
}

pub struct EmbeddedSessionClientArgs {
    pub auth_manager: Arc<AuthManager>,
    pub thread_manager: Arc<ThreadManager>,
    pub config: Config,
    pub cli_overrides: Vec<(String, TomlValue)>,
    pub feedback: CodexFeedback,
    pub codex_linux_sandbox_exe: Option<PathBuf>,
}

pub struct EmbeddedSessionClient {
    input_tx: mpsc::Sender<EmbeddedSessionInput>,
    output_rx: mpsc::Receiver<JSONRPCMessage>,
}

impl EmbeddedSessionClient {
    pub fn spawn(args: EmbeddedSessionClientArgs) -> Self {
        let (input_tx, mut input_rx) = mpsc::channel::<EmbeddedSessionInput>(CHANNEL_CAPACITY);
        let (output_tx, output_rx) = mpsc::channel::<JSONRPCMessage>(CHANNEL_CAPACITY);
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<OutgoingEnvelope>(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(outgoing_tx));

        let EmbeddedSessionClientArgs {
            auth_manager,
            thread_manager,
            config,
            cli_overrides,
            feedback,
            codex_linux_sandbox_exe,
        } = args;
        let config = Arc::new(config);
        let cloud_requirements = Arc::new(std::sync::RwLock::new(cloud_requirements_loader(
            auth_manager.clone(),
            config.chatgpt_base_url.clone(),
            config.codex_home.clone(),
        )));

        let mut processor = CodexMessageProcessor::new(CodexMessageProcessorArgs {
            auth_manager,
            thread_manager,
            outgoing: outgoing.clone(),
            codex_linux_sandbox_exe,
            config,
            cli_overrides,
            cloud_requirements,
            single_client_mode: true,
            feedback,
        });

        tokio::spawn(async move {
            while let Some(envelope) = outgoing_rx.recv().await {
                let message = match envelope {
                    OutgoingEnvelope::ToConnection {
                        connection_id,
                        message,
                    } => {
                        if connection_id != EMBEDDED_CONNECTION_ID {
                            continue;
                        }
                        outgoing_message_to_jsonrpc(message)
                    }
                    OutgoingEnvelope::Broadcast { message } => outgoing_message_to_jsonrpc(message),
                };

                if output_tx.send(message).await.is_err() {
                    break;
                }
            }
        });

        tokio::spawn(async move {
            while let Some(input) = input_rx.recv().await {
                match input {
                    EmbeddedSessionInput::Request(request) => {
                        processor
                            .process_request(EMBEDDED_CONNECTION_ID, request)
                            .await;
                    }
                    EmbeddedSessionInput::Response(response) => {
                        outgoing
                            .notify_client_response(response.id, response.result)
                            .await;
                    }
                    EmbeddedSessionInput::Error(error) => {
                        outgoing.notify_client_error(error.id, error.error).await;
                    }
                }
            }
            processor.connection_closed(EMBEDDED_CONNECTION_ID).await;
        });

        Self {
            input_tx,
            output_rx,
        }
    }

    pub async fn send_request(&self, request: ClientRequest) -> std::io::Result<()> {
        self.input_tx
            .send(EmbeddedSessionInput::Request(request))
            .await
            .map_err(|_| std::io::Error::other("embedded app-server session is closed"))
    }

    pub async fn send_response(&self, response: JSONRPCResponse) -> std::io::Result<()> {
        self.input_tx
            .send(EmbeddedSessionInput::Response(response))
            .await
            .map_err(|_| std::io::Error::other("embedded app-server session is closed"))
    }

    pub async fn send_error(&self, error: JSONRPCError) -> std::io::Result<()> {
        self.input_tx
            .send(EmbeddedSessionInput::Error(error))
            .await
            .map_err(|_| std::io::Error::other("embedded app-server session is closed"))
    }

    pub async fn recv(&mut self) -> Option<JSONRPCMessage> {
        self.output_rx.recv().await
    }
}

fn outgoing_message_to_jsonrpc(message: OutgoingMessage) -> JSONRPCMessage {
    let value =
        serde_json::to_value(message).expect("outgoing app-server message should serialize");
    serde_json::from_value(value).expect("outgoing app-server message should decode as JSON-RPC")
}
