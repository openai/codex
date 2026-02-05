use std::path::PathBuf;
use std::sync::Arc;

use crate::codex_message_processor::CodexMessageProcessor;
use crate::codex_message_processor::CodexMessageProcessorArgs;
use crate::config_api::ConfigApi;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use async_trait::async_trait;
use codex_app_server_protocol::ChatgptAuthTokensRefreshParams;
use codex_app_server_protocol::ChatgptAuthTokensRefreshReason;
use codex_app_server_protocol::ChatgptAuthTokensRefreshResponse;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigReadParams;
use codex_app_server_protocol::ConfigValueWriteParams;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::ExperimentalApi;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequestPayload;
use codex_app_server_protocol::experimental_required_message;
use codex_core::AuthManager;
use codex_core::ThreadManager;
use codex_core::auth::ExternalAuthRefreshContext;
use codex_core::auth::ExternalAuthRefreshReason;
use codex_core::auth::ExternalAuthRefresher;
use codex_core::auth::ExternalAuthTokens;
use codex_core::config::Config;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::LoaderOverrides;
use codex_core::default_client::SetOriginatorError;
use codex_core::default_client::USER_AGENT_SUFFIX;
use codex_core::default_client::get_codex_user_agent;
use codex_core::default_client::set_default_client_residency_requirement;
use codex_core::default_client::set_default_originator;
use codex_feedback::CodexFeedback;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use tokio::sync::broadcast;
use tokio::time::Duration;
use tokio::time::timeout;
use toml::Value as TomlValue;

const EXTERNAL_AUTH_REFRESH_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct ExternalAuthRefreshBridge {
    outgoing: Arc<OutgoingMessageSender>,
}

impl ExternalAuthRefreshBridge {
    fn map_reason(reason: ExternalAuthRefreshReason) -> ChatgptAuthTokensRefreshReason {
        match reason {
            ExternalAuthRefreshReason::Unauthorized => ChatgptAuthTokensRefreshReason::Unauthorized,
        }
    }
}

#[async_trait]
impl ExternalAuthRefresher for ExternalAuthRefreshBridge {
    async fn refresh(
        &self,
        context: ExternalAuthRefreshContext,
    ) -> std::io::Result<ExternalAuthTokens> {
        let params = ChatgptAuthTokensRefreshParams {
            reason: Self::map_reason(context.reason),
            previous_account_id: context.previous_account_id,
        };

        let (request_id, rx) = self
            .outgoing
            .send_request_with_id(ServerRequestPayload::ChatgptAuthTokensRefresh(params))
            .await;

        let result = match timeout(EXTERNAL_AUTH_REFRESH_TIMEOUT, rx).await {
            Ok(result) => result.map_err(|err| {
                std::io::Error::other(format!("auth refresh request canceled: {err}"))
            })?,
            Err(_) => {
                let _canceled = self.outgoing.cancel_request(&request_id).await;
                return Err(std::io::Error::other(format!(
                    "auth refresh request timed out after {}s",
                    EXTERNAL_AUTH_REFRESH_TIMEOUT.as_secs()
                )));
            }
        };

        let response: ChatgptAuthTokensRefreshResponse =
            serde_json::from_value(result).map_err(std::io::Error::other)?;

        Ok(ExternalAuthTokens {
            access_token: response.access_token,
            id_token: response.id_token,
        })
    }
}

pub(crate) struct MessageProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    codex_message_processor: CodexMessageProcessor,
    config_api: ConfigApi,
    config: Arc<Config>,
    config_warnings: Arc<Vec<ConfigWarningNotification>>,
}

#[derive(Debug, Default)]
pub(crate) struct ConnectionSessionState {
    pub(crate) initialized: bool,
    experimental_api_enabled: bool,
}

pub(crate) struct MessageProcessorArgs {
    pub(crate) outgoing: Arc<OutgoingMessageSender>,
    pub(crate) codex_linux_sandbox_exe: Option<PathBuf>,
    pub(crate) config: Arc<Config>,
    pub(crate) cli_overrides: Vec<(String, TomlValue)>,
    pub(crate) loader_overrides: LoaderOverrides,
    pub(crate) cloud_requirements: CloudRequirementsLoader,
    pub(crate) feedback: CodexFeedback,
    pub(crate) config_warnings: Vec<ConfigWarningNotification>,
}

impl MessageProcessor {
    /// Create a new `MessageProcessor`, retaining a handle to the outgoing
    /// `Sender` so handlers can enqueue messages to be written to stdout.
    pub(crate) fn new(args: MessageProcessorArgs) -> Self {
        let MessageProcessorArgs {
            outgoing,
            codex_linux_sandbox_exe,
            config,
            cli_overrides,
            loader_overrides,
            cloud_requirements,
            feedback,
            config_warnings,
        } = args;
        let auth_manager = AuthManager::shared(
            config.codex_home.clone(),
            false,
            config.cli_auth_credentials_store_mode,
        );
        auth_manager.set_forced_chatgpt_workspace_id(config.forced_chatgpt_workspace_id.clone());
        auth_manager.set_external_auth_refresher(Arc::new(ExternalAuthRefreshBridge {
            outgoing: outgoing.clone(),
        }));
        let thread_manager = Arc::new(ThreadManager::new(
            config.codex_home.clone(),
            auth_manager.clone(),
            SessionSource::VSCode,
        ));
        let codex_message_processor = CodexMessageProcessor::new(CodexMessageProcessorArgs {
            auth_manager,
            thread_manager,
            outgoing: outgoing.clone(),
            codex_linux_sandbox_exe,
            config: Arc::clone(&config),
            cli_overrides: cli_overrides.clone(),
            cloud_requirements: cloud_requirements.clone(),
            feedback,
        });
        let config_api = ConfigApi::new(
            config.codex_home.clone(),
            cli_overrides,
            loader_overrides,
            cloud_requirements,
        );

        Self {
            outgoing,
            codex_message_processor,
            config_api,
            config,
            config_warnings: Arc::new(config_warnings),
        }
    }

    pub(crate) async fn process_request(
        &mut self,
        connection_id: ConnectionId,
        request: JSONRPCRequest,
        session: &mut ConnectionSessionState,
    ) {
        let request_id = ConnectionRequestId {
            connection_id,
            request_id: request.id.clone(),
        };
        let request_json = match serde_json::to_value(&request) {
            Ok(request_json) => request_json,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("Invalid request: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        let codex_request = match serde_json::from_value::<ClientRequest>(request_json) {
            Ok(codex_request) => codex_request,
            Err(err) => {
                let error = JSONRPCErrorError {
                    code: INVALID_REQUEST_ERROR_CODE,
                    message: format!("Invalid request: {err}"),
                    data: None,
                };
                self.outgoing.send_error(request_id, error).await;
                return;
            }
        };

        match codex_request {
            // Handle Initialize internally so CodexMessageProcessor does not have to concern
            // itself with the `initialized` bool.
            ClientRequest::Initialize { request_id, params } => {
                let request_id = ConnectionRequestId {
                    connection_id,
                    request_id,
                };
                if session.initialized {
                    let error = JSONRPCErrorError {
                        code: INVALID_REQUEST_ERROR_CODE,
                        message: "Already initialized".to_string(),
                        data: None,
                    };
                    self.outgoing.send_error(request_id, error).await;
                    return;
                } else {
                    // TODO(maxj): Revisit capability scoping for `experimental_api_enabled`.
                    // Current behavior is per-connection. Reviewer feedback notes this can
                    // create odd cross-client behavior (for example dynamic tool calls on a
                    // shared thread when another connected client did not opt into
                    // experimental API). Proposed direction is instance-global first-write-wins
                    // with initialize-time mismatch rejection.
                    session.experimental_api_enabled = params
                        .capabilities
                        .as_ref()
                        .is_some_and(|cap| cap.experimental_api);
                    let ClientInfo {
                        name,
                        title: _title,
                        version,
                    } = params.client_info;
                    if let Err(error) = set_default_originator(name.clone()) {
                        match error {
                            SetOriginatorError::InvalidHeaderValue => {
                                let error = JSONRPCErrorError {
                                    code: INVALID_REQUEST_ERROR_CODE,
                                    message: format!(
                                        "Invalid clientInfo.name: '{name}'. Must be a valid HTTP header value."
                                    ),
                                    data: None,
                                };
                                self.outgoing.send_error(request_id.clone(), error).await;
                                return;
                            }
                            SetOriginatorError::AlreadyInitialized => {
                                // No-op. This is expected to happen if the originator is already set via env var.
                                // TODO(owen): Once we remove support for CODEX_INTERNAL_ORIGINATOR_OVERRIDE,
                                // this will be an unexpected state and we can return a JSON-RPC error indicating
                                // internal server error.
                            }
                        }
                    }
                    set_default_client_residency_requirement(self.config.enforce_residency.value());
                    let user_agent_suffix = format!("{name}; {version}");
                    if let Ok(mut suffix) = USER_AGENT_SUFFIX.lock() {
                        *suffix = Some(user_agent_suffix);
                    }

                    let user_agent = get_codex_user_agent();
                    let response = InitializeResponse { user_agent };
                    self.outgoing.send_response(request_id, response).await;

                    session.initialized = true;
                    for notification in self.config_warnings.iter().cloned() {
                        self.outgoing
                            .send_server_notification(ServerNotification::ConfigWarning(
                                notification,
                            ))
                            .await;
                    }

                    return;
                }
            }
            _ => {
                if !session.initialized {
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

        if let Some(reason) = codex_request.experimental_reason()
            && !session.experimental_api_enabled
        {
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: experimental_required_message(reason),
                data: None,
            };
            self.outgoing.send_error(request_id, error).await;
            return;
        }

        match codex_request {
            ClientRequest::ConfigRead { request_id, params } => {
                self.handle_config_read(
                    ConnectionRequestId {
                        connection_id,
                        request_id,
                    },
                    params,
                )
                .await;
            }
            ClientRequest::ConfigValueWrite { request_id, params } => {
                self.handle_config_value_write(
                    ConnectionRequestId {
                        connection_id,
                        request_id,
                    },
                    params,
                )
                .await;
            }
            ClientRequest::ConfigBatchWrite { request_id, params } => {
                self.handle_config_batch_write(
                    ConnectionRequestId {
                        connection_id,
                        request_id,
                    },
                    params,
                )
                .await;
            }
            ClientRequest::ConfigRequirementsRead {
                request_id,
                params: _,
            } => {
                self.handle_config_requirements_read(ConnectionRequestId {
                    connection_id,
                    request_id,
                })
                .await;
            }
            other => {
                self.codex_message_processor
                    .process_request(connection_id, other)
                    .await;
            }
        }
    }

    pub(crate) async fn process_notification(&self, notification: JSONRPCNotification) {
        // Currently, we do not expect to receive any notifications from the
        // client, so we just log them.
        tracing::info!("<- notification: {:?}", notification);
    }

    pub(crate) fn thread_created_receiver(&self) -> broadcast::Receiver<ThreadId> {
        self.codex_message_processor.thread_created_receiver()
    }

    pub(crate) async fn try_attach_thread_listener(&mut self, thread_id: ThreadId) {
        self.codex_message_processor
            .try_attach_thread_listener(thread_id)
            .await;
    }

    /// Handle a standalone JSON-RPC response originating from the peer.
    pub(crate) async fn process_response(&mut self, response: JSONRPCResponse) {
        tracing::info!("<- response: {:?}", response);
        let JSONRPCResponse { id, result, .. } = response;
        self.outgoing.notify_client_response(id, result).await
    }

    /// Handle an error object received from the peer.
    pub(crate) async fn process_error(&mut self, err: JSONRPCError) {
        tracing::error!("<- error: {:?}", err);
        self.outgoing.notify_client_error(err.id, err.error).await;
    }

    async fn handle_config_read(&self, request_id: ConnectionRequestId, params: ConfigReadParams) {
        match self.config_api.read(params).await {
            Ok(response) => self.outgoing.send_response(request_id, response).await,
            Err(error) => self.outgoing.send_error(request_id, error).await,
        }
    }

    async fn handle_config_value_write(
        &self,
        request_id: ConnectionRequestId,
        params: ConfigValueWriteParams,
    ) {
        match self.config_api.write_value(params).await {
            Ok(response) => self.outgoing.send_response(request_id, response).await,
            Err(error) => self.outgoing.send_error(request_id, error).await,
        }
    }

    async fn handle_config_batch_write(
        &self,
        request_id: ConnectionRequestId,
        params: ConfigBatchWriteParams,
    ) {
        match self.config_api.batch_write(params).await {
            Ok(response) => self.outgoing.send_response(request_id, response).await,
            Err(error) => self.outgoing.send_error(request_id, error).await,
        }
    }

    async fn handle_config_requirements_read(&self, request_id: ConnectionRequestId) {
        match self.config_api.config_requirements_read().await {
            Ok(response) => self.outgoing.send_response(request_id, response).await,
            Err(error) => self.outgoing.send_error(request_id, error).await,
        }
    }
}
