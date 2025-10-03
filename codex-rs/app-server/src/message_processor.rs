use std::path::PathBuf;

use crate::codex_message_processor::CodexMessageProcessor;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeResponse;

use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::default_client::USER_AGENT_SUFFIX;
use codex_core::default_client::get_codex_user_agent;
use codex_protocol::protocol::SessionSource;
use std::sync::Arc;

pub(crate) struct MessageProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    codex_message_processor: CodexMessageProcessor,
    initialized: bool,
}

impl MessageProcessor {
    /// Create a new `MessageProcessor`, retaining a handle to the outgoing
    /// `Sender` so handlers can enqueue messages to be written to stdout.
    pub(crate) fn new(
        outgoing: OutgoingMessageSender,
        codex_linux_sandbox_exe: Option<PathBuf>,
        config: Arc<Config>,
    ) -> Self {
        let outgoing = Arc::new(outgoing);
        let auth_manager = AuthManager::shared(config.codex_home.clone(), false);
        let conversation_manager = Arc::new(ConversationManager::new(
            auth_manager.clone(),
            SessionSource::VSCode,
        ));
        let codex_message_processor = CodexMessageProcessor::new(
            auth_manager,
            conversation_manager,
            outgoing.clone(),
            codex_linux_sandbox_exe,
            config,
        );

        Self {
            outgoing,
            codex_message_processor,
            initialized: false,
        }
    }

    pub(crate) async fn process_request(&mut self, request: JSONRPCRequest) {
        let request_id = request.id.clone();
        if let Ok(request_json) = serde_json::to_value(request)
            && let Ok(codex_request) = serde_json::from_value::<ClientRequest>(request_json)
        {
            match codex_request {
                // Handle Initialize internally so CodexMessageProcessor does not have to concern
                // itself with the `initialized` bool.
                ClientRequest::Initialize { request_id, params } => {
                    if self.initialized {
                        let error = JSONRPCErrorError {
                            code: INVALID_REQUEST_ERROR_CODE,
                            message: "Already initialized".to_string(),
                            data: None,
                        };
                        self.outgoing.send_error(request_id, error).await;
                        return;
                    } else {
                        let ClientInfo {
                            name,
                            title: _title,
                            version,
                        } = params.client_info;
                        let user_agent_suffix = format!("{name}; {version}");
                        if let Ok(mut suffix) = USER_AGENT_SUFFIX.lock() {
                            *suffix = Some(user_agent_suffix);
                        }

                        let user_agent = get_codex_user_agent();
                        let response = InitializeResponse { user_agent };
                        self.outgoing.send_response(request_id, response).await;

                        self.initialized = true;
                        return;
                    }
                }
                _ => {
                    if !self.initialized {
                        let error = JSONRPCErrorError {
                            code: INVALID_REQUEST_ERROR_CODE,
                            message: "Not initialized".to_string(),
                            data: None,
                        };
                        self.outgoing.send_error(request_id, error).await;
                        return;
                    }
                }
            }

            self.codex_message_processor
                .process_request(codex_request)
                .await;
        } else {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "Invalid request".to_string(),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
        }
    }

    pub(crate) async fn process_notification(&self, notification: JSONRPCNotification) {
        // Currently, we do not expect to receive any notifications from the
        // client, so we just log them.
        tracing::info!("<- notification: {:?}", notification);
    }

    /// Handle a standalone JSON-RPC response originating from the peer.
    pub(crate) async fn process_response(&mut self, response: JSONRPCResponse) {
        tracing::info!("<- response: {:?}", response);
        let JSONRPCResponse { id, result, .. } = response;
        self.outgoing.notify_client_response(id, result).await
    }

    /// Handle an error object received from the peer.
    pub(crate) fn process_error(&mut self, err: JSONRPCError) {
        tracing::error!("<- error: {:?}", err);
    }
}
