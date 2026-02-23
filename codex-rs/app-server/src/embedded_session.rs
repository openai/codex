use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadForkParams;
use codex_app_server_protocol::ThreadForkResponse;
use codex_app_server_protocol::ThreadResumeParams;
use codex_app_server_protocol::ThreadResumeResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_cloud_requirements::cloud_requirements_loader;
use codex_core::AuthManager;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_feedback::CodexFeedback;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use toml::Value as TomlValue;
use tracing::warn;

use crate::codex_message_processor::CodexMessageProcessor;
use crate::codex_message_processor::CodexMessageProcessorArgs;
use crate::error_code::INTERNAL_ERROR_CODE;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::OutgoingMessageSender;
use crate::transport::CHANNEL_CAPACITY;

const EMBEDDED_CONNECTION_ID: ConnectionId = ConnectionId(1);

#[derive(Debug, Clone)]
pub struct EmbeddedServerRequest {
    pub request_id: RequestId,
    pub request: ServerRequest,
}

#[derive(Debug, Clone)]
pub enum EmbeddedSessionMessage {
    Notification(ServerNotification),
    Request(EmbeddedServerRequest),
}

#[derive(Clone)]
pub struct EmbeddedSessionClient {
    inbound_tx: mpsc::Sender<EmbeddedInbound>,
    messages_tx: broadcast::Sender<EmbeddedSessionMessage>,
    next_request_id: Arc<AtomicI64>,
}

pub struct EmbeddedSessionClientArgs {
    pub auth_manager: Arc<AuthManager>,
    pub thread_manager: Arc<ThreadManager>,
    pub config: Config,
    pub cli_overrides: Vec<(String, TomlValue)>,
    pub feedback: CodexFeedback,
}

enum EmbeddedInbound {
    ClientRequest {
        request: ClientRequest,
        reply_tx: oneshot::Sender<std::result::Result<serde_json::Value, JSONRPCErrorError>>,
    },
    ServerResponse {
        request_id: RequestId,
        result: serde_json::Value,
    },
    ServerError {
        request_id: RequestId,
        error: JSONRPCErrorError,
    },
}

impl EmbeddedSessionClient {
    pub fn new(args: EmbeddedSessionClientArgs) -> Self {
        let EmbeddedSessionClientArgs {
            auth_manager,
            thread_manager,
            config,
            cli_overrides,
            feedback,
        } = args;
        let config = Arc::new(config);
        let cloud_requirements = cloud_requirements_loader(
            auth_manager.clone(),
            config.chatgpt_base_url.clone(),
            config.codex_home.clone(),
        );

        let (outgoing_tx, outgoing_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let outgoing = Arc::new(OutgoingMessageSender::new(outgoing_tx));
        let processor = CodexMessageProcessor::new(CodexMessageProcessorArgs {
            auth_manager,
            thread_manager,
            outgoing: outgoing.clone(),
            codex_linux_sandbox_exe: config.codex_linux_sandbox_exe.clone(),
            config: config.clone(),
            cli_overrides,
            cloud_requirements: Arc::new(RwLock::new(cloud_requirements)),
            single_client_mode: true,
            feedback,
        });

        let (inbound_tx, inbound_rx) = mpsc::channel(CHANNEL_CAPACITY);
        let (messages_tx, _) = broadcast::channel(CHANNEL_CAPACITY);

        tokio::spawn(run_embedded_session_loop(
            processor,
            outgoing,
            outgoing_rx,
            inbound_rx,
            messages_tx.clone(),
        ));

        Self {
            inbound_tx,
            messages_tx,
            next_request_id: Arc::new(AtomicI64::new(1)),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EmbeddedSessionMessage> {
        self.messages_tx.subscribe()
    }

    pub async fn thread_start(
        &self,
        params: ThreadStartParams,
    ) -> std::result::Result<ThreadStartResponse, JSONRPCErrorError> {
        self.request(|request_id| ClientRequest::ThreadStart { request_id, params })
            .await
    }

    pub async fn thread_resume(
        &self,
        params: ThreadResumeParams,
    ) -> std::result::Result<ThreadResumeResponse, JSONRPCErrorError> {
        self.request(|request_id| ClientRequest::ThreadResume { request_id, params })
            .await
    }

    pub async fn thread_fork(
        &self,
        params: ThreadForkParams,
    ) -> std::result::Result<ThreadForkResponse, JSONRPCErrorError> {
        self.request(|request_id| ClientRequest::ThreadFork { request_id, params })
            .await
    }

    pub async fn respond<T: Serialize>(
        &self,
        request_id: RequestId,
        response: T,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        let result = serde_json::to_value(response).map_err(serialize_error)?;
        self.inbound_tx
            .send(EmbeddedInbound::ServerResponse { request_id, result })
            .await
            .map_err(channel_closed_error)?;
        Ok(())
    }

    pub async fn respond_error(
        &self,
        request_id: RequestId,
        error: JSONRPCErrorError,
    ) -> std::result::Result<(), JSONRPCErrorError> {
        self.inbound_tx
            .send(EmbeddedInbound::ServerError { request_id, error })
            .await
            .map_err(channel_closed_error)?;
        Ok(())
    }

    pub async fn request<T, F>(&self, build: F) -> std::result::Result<T, JSONRPCErrorError>
    where
        T: DeserializeOwned,
        F: FnOnce(RequestId) -> ClientRequest,
    {
        let request_id = RequestId::Integer(self.next_request_id.fetch_add(1, Ordering::Relaxed));
        let request = build(request_id);
        let (reply_tx, reply_rx) = oneshot::channel();
        self.inbound_tx
            .send(EmbeddedInbound::ClientRequest { request, reply_tx })
            .await
            .map_err(channel_closed_error)?;

        let response = reply_rx.await.map_err(channel_closed_error)??;
        serde_json::from_value(response).map_err(serialize_error)
    }
}

async fn run_embedded_session_loop(
    mut processor: CodexMessageProcessor,
    outgoing: Arc<OutgoingMessageSender>,
    mut outgoing_rx: mpsc::Receiver<OutgoingEnvelope>,
    mut inbound_rx: mpsc::Receiver<EmbeddedInbound>,
    messages_tx: broadcast::Sender<EmbeddedSessionMessage>,
) {
    let mut pending_client_requests: HashMap<
        RequestId,
        oneshot::Sender<std::result::Result<serde_json::Value, JSONRPCErrorError>>,
    > = HashMap::new();

    loop {
        tokio::select! {
            maybe_inbound = inbound_rx.recv() => {
                let Some(inbound) = maybe_inbound else {
                    break;
                };
                match inbound {
                    EmbeddedInbound::ClientRequest { request, reply_tx } => {
                        let request_id = match extract_client_request_id(&request) {
                            Ok(request_id) => request_id,
                            Err(err) => {
                                let _ = reply_tx.send(Err(err));
                                continue;
                            }
                        };
                        pending_client_requests.insert(request_id, reply_tx);
                        processor.process_request(EMBEDDED_CONNECTION_ID, request).await;
                    }
                    EmbeddedInbound::ServerResponse { request_id, result } => {
                        outgoing.notify_client_response(request_id, result).await;
                    }
                    EmbeddedInbound::ServerError { request_id, error } => {
                        outgoing.notify_client_error(request_id, error).await;
                    }
                }
            }
            maybe_outgoing = outgoing_rx.recv() => {
                let Some(envelope) = maybe_outgoing else {
                    break;
                };
                let message = match envelope {
                    OutgoingEnvelope::ToConnection { connection_id, message } => {
                        if connection_id != EMBEDDED_CONNECTION_ID {
                            continue;
                        }
                        message
                    }
                    OutgoingEnvelope::Broadcast { message } => message,
                };
                match message {
                    OutgoingMessage::Response(response) => {
                        if let Some(reply_tx) = pending_client_requests.remove(&response.id) {
                            let _ = reply_tx.send(Ok(response.result));
                        } else {
                            warn!("embedded session dropped unmatched response {:?}", response.id);
                        }
                    }
                    OutgoingMessage::Error(error) => {
                        if let Some(reply_tx) = pending_client_requests.remove(&error.id) {
                            let _ = reply_tx.send(Err(error.error));
                        } else {
                            warn!("embedded session dropped unmatched error {:?}", error.id);
                        }
                    }
                    OutgoingMessage::AppServerNotification(notification) => {
                        let _ = messages_tx.send(EmbeddedSessionMessage::Notification(notification));
                    }
                    OutgoingMessage::Request(request) => match extract_server_request_id(&request) {
                        Ok(request_id) => {
                            let _ = messages_tx.send(EmbeddedSessionMessage::Request(
                                EmbeddedServerRequest { request_id, request },
                            ));
                        }
                        Err(err) => {
                            warn!("failed to extract embedded server request id: {err:?}");
                        }
                    },
                    OutgoingMessage::Notification(_legacy_notification) => {
                        // Embedded TUI clients should rely on typed app-server notifications.
                    }
                }
            }
        }
    }
}

fn extract_client_request_id(
    request: &ClientRequest,
) -> std::result::Result<RequestId, JSONRPCErrorError> {
    extract_request_id(serde_json::to_value(request).map_err(serialize_error)?)
}

fn extract_server_request_id(
    request: &ServerRequest,
) -> std::result::Result<RequestId, JSONRPCErrorError> {
    extract_request_id(serde_json::to_value(request).map_err(serialize_error)?)
}

fn extract_request_id(
    value: serde_json::Value,
) -> std::result::Result<RequestId, JSONRPCErrorError> {
    let id = value
        .get("id")
        .cloned()
        .ok_or_else(|| internal_error("missing id on request".to_string()))?;
    serde_json::from_value(id).map_err(serialize_error)
}

fn serialize_error(err: serde_json::Error) -> JSONRPCErrorError {
    internal_error(format!("serialization error: {err}"))
}

fn channel_closed_error<T>(err: T) -> JSONRPCErrorError
where
    T: std::fmt::Display,
{
    internal_error(format!("embedded session channel closed: {err}"))
}

fn internal_error(message: String) -> JSONRPCErrorError {
    JSONRPCErrorError {
        code: INTERNAL_ERROR_CODE,
        message,
        data: None,
    }
}
