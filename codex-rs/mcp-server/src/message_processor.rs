use std::collections::HashMap;
use std::path::PathBuf;

use crate::codex_tool_config::CodexToolCallParam;
use crate::codex_tool_config::CodexToolCallReplyParam;
use crate::codex_tool_config::create_tool_for_codex_tool_call_param;
use crate::codex_tool_config::create_tool_for_codex_tool_call_reply_param;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::outgoing_message::OutgoingMessageSender;
use codex_protocol::ConversationId;
use codex_protocol::protocol::SessionSource;

use codex_core::AuthManager;
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::default_client::USER_AGENT_SUFFIX;
use codex_core::default_client::get_codex_user_agent;
use codex_core::protocol::Submission;
use mcp_types::CallToolRequestParams;
use mcp_types::CallToolResult;
use mcp_types::ClientRequest as McpClientRequest;
use mcp_types::ContentBlock;
use mcp_types::JSONRPCError;
use mcp_types::JSONRPCErrorError;
use mcp_types::JSONRPCNotification;
use mcp_types::JSONRPCRequest;
use mcp_types::JSONRPCResponse;
use mcp_types::ListToolsResult;
use mcp_types::ModelContextProtocolRequest;
use mcp_types::RequestId;
use mcp_types::ServerCapabilitiesTools;
use mcp_types::ServerNotification;
use mcp_types::TextContent;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task;

pub(crate) struct MessageProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    initialized: bool,
    codex_linux_sandbox_exe: Option<PathBuf>,
    conversation_manager: Arc<ConversationManager>,
    running_requests_id_to_codex_uuid: Arc<Mutex<HashMap<RequestId, ConversationId>>>,
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
        let conversation_manager =
            Arc::new(ConversationManager::new(auth_manager, SessionSource::Mcp));
        Self {
            outgoing,
            initialized: false,
            codex_linux_sandbox_exe,
            conversation_manager,
            running_requests_id_to_codex_uuid: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) async fn process_request(&mut self, request: JSONRPCRequest) {
        // Hold on to the ID so we can respond.
        let request_id = request.id.clone();

        let client_request = match McpClientRequest::try_from(request) {
            Ok(client_request) => client_request,
            Err(e) => {
                tracing::warn!("Failed to convert request: {e}");
                return;
            }
        };

        // Dispatch to a dedicated handler for each request type.
        match client_request {
            McpClientRequest::InitializeRequest(params) => {
                self.handle_initialize(request_id, params).await;
            }
            McpClientRequest::PingRequest(params) => {
                self.handle_ping(request_id, params).await;
            }
            McpClientRequest::ListResourcesRequest(params) => {
                self.handle_list_resources(params);
            }
            McpClientRequest::ListResourceTemplatesRequest(params) => {
                self.handle_list_resource_templates(params);
            }
            McpClientRequest::ReadResourceRequest(params) => {
                self.handle_read_resource(params);
            }
            McpClientRequest::SubscribeRequest(params) => {
                self.handle_subscribe(params);
            }
            McpClientRequest::UnsubscribeRequest(params) => {
                self.handle_unsubscribe(params);
            }
            McpClientRequest::ListPromptsRequest(params) => {
                self.handle_list_prompts(params);
            }
            McpClientRequest::GetPromptRequest(params) => {
                self.handle_get_prompt(params);
            }
            McpClientRequest::ListToolsRequest(params) => {
                self.handle_list_tools(request_id, params).await;
            }
            McpClientRequest::CallToolRequest(params) => {
                self.handle_call_tool(request_id, params).await;
            }
            McpClientRequest::SetLevelRequest(params) => {
                self.handle_set_level(params);
            }
            McpClientRequest::CompleteRequest(params) => {
                self.handle_complete(params);
            }
        }
    }

    /// Handle a standalone JSON-RPC response originating from the peer.
    pub(crate) async fn process_response(&mut self, response: JSONRPCResponse) {
        tracing::info!("<- response: {:?}", response);
        let JSONRPCResponse { id, result, .. } = response;
        self.outgoing.notify_client_response(id, result).await
    }

    /// Handle a fire-and-forget JSON-RPC notification.
    pub(crate) async fn process_notification(&mut self, notification: JSONRPCNotification) {
        let server_notification = match ServerNotification::try_from(notification) {
            Ok(n) => n,
            Err(e) => {
                tracing::warn!("Failed to convert notification: {e}");
                return;
            }
        };

        // Similar to requests, route each notification type to its own stub
        // handler so additional logic can be implemented incrementally.
        match server_notification {
            ServerNotification::CancelledNotification(params) => {
                self.handle_cancelled_notification(params).await;
            }
            ServerNotification::ProgressNotification(params) => {
                self.handle_progress_notification(params);
            }
            ServerNotification::ResourceListChangedNotification(params) => {
                self.handle_resource_list_changed(params);
            }
            ServerNotification::ResourceUpdatedNotification(params) => {
                self.handle_resource_updated(params);
            }
            ServerNotification::PromptListChangedNotification(params) => {
                self.handle_prompt_list_changed(params);
            }
            ServerNotification::ToolListChangedNotification(params) => {
                self.handle_tool_list_changed(params);
            }
            ServerNotification::LoggingMessageNotification(params) => {
                self.handle_logging_message(params);
            }
        }
    }

    /// Handle an error object received from the peer.
    pub(crate) fn process_error(&mut self, err: JSONRPCError) {
        tracing::error!("<- error: {:?}", err);
    }

    async fn handle_initialize(
        &mut self,
        id: RequestId,
        params: <mcp_types::InitializeRequest as ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("initialize -> params: {:?}", params);

        if self.initialized {
            // Already initialised: send JSON-RPC error response.
            let error = JSONRPCErrorError {
                code: INVALID_REQUEST_ERROR_CODE,
                message: "initialize called more than once".to_string(),
                data: None,
            };
            self.outgoing.send_error(id, error).await;
            return;
        }

        let client_info = params.client_info;
        let name = client_info.name;
        let version = client_info.version;
        let user_agent_suffix = format!("{name}; {version}");
        if let Ok(mut suffix) = USER_AGENT_SUFFIX.lock() {
            *suffix = Some(user_agent_suffix);
        }

        self.initialized = true;

        // Build a minimal InitializeResult. Fill with placeholders.
        let result = mcp_types::InitializeResult {
            capabilities: mcp_types::ServerCapabilities {
                completions: None,
                experimental: None,
                logging: None,
                prompts: None,
                resources: None,
                tools: Some(ServerCapabilitiesTools {
                    list_changed: Some(true),
                }),
            },
            instructions: None,
            protocol_version: params.protocol_version.clone(),
            server_info: mcp_types::Implementation {
                name: "codex-mcp-server".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Codex".to_string()),
                user_agent: Some(get_codex_user_agent()),
            },
        };

        self.send_response::<mcp_types::InitializeRequest>(id, result)
            .await;
    }

    async fn send_response<T>(&self, id: RequestId, result: T::Result)
    where
        T: ModelContextProtocolRequest,
    {
        self.outgoing.send_response(id, result).await;
    }

    async fn handle_ping(
        &self,
        id: RequestId,
        params: <mcp_types::PingRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("ping -> params: {:?}", params);
        let result = json!({});
        self.send_response::<mcp_types::PingRequest>(id, result)
            .await;
    }

    fn handle_list_resources(
        &self,
        params: <mcp_types::ListResourcesRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("resources/list -> params: {:?}", params);
    }

    fn handle_list_resource_templates(
        &self,
        params:
            <mcp_types::ListResourceTemplatesRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("resources/templates/list -> params: {:?}", params);
    }

    fn handle_read_resource(
        &self,
        params: <mcp_types::ReadResourceRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("resources/read -> params: {:?}", params);
    }

    fn handle_subscribe(
        &self,
        params: <mcp_types::SubscribeRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("resources/subscribe -> params: {:?}", params);
    }

    fn handle_unsubscribe(
        &self,
        params: <mcp_types::UnsubscribeRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("resources/unsubscribe -> params: {:?}", params);
    }

    fn handle_list_prompts(
        &self,
        params: <mcp_types::ListPromptsRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("prompts/list -> params: {:?}", params);
    }

    fn handle_get_prompt(
        &self,
        params: <mcp_types::GetPromptRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("prompts/get -> params: {:?}", params);
    }

    async fn handle_list_tools(
        &self,
        id: RequestId,
        params: <mcp_types::ListToolsRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::trace!("tools/list -> {params:?}");
        let result = ListToolsResult {
            tools: vec![
                create_tool_for_codex_tool_call_param(),
                create_tool_for_codex_tool_call_reply_param(),
            ],
            next_cursor: None,
        };

        self.send_response::<mcp_types::ListToolsRequest>(id, result)
            .await;
    }

    async fn handle_call_tool(
        &self,
        id: RequestId,
        params: <mcp_types::CallToolRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("tools/call -> params: {:?}", params);
        let CallToolRequestParams { name, arguments } = params;

        match name.as_str() {
            "codex" => self.handle_tool_call_codex(id, arguments).await,
            "codex-reply" => {
                self.handle_tool_call_codex_session_reply(id, arguments)
                    .await
            }
            _ => {
                let result = CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: format!("Unknown tool '{name}'"),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: None,
                };
                self.send_response::<mcp_types::CallToolRequest>(id, result)
                    .await;
            }
        }
    }
    async fn handle_tool_call_codex(&self, id: RequestId, arguments: Option<serde_json::Value>) {
        let (initial_prompt, config): (String, Config) = match arguments {
            Some(json_val) => match serde_json::from_value::<CodexToolCallParam>(json_val) {
                Ok(tool_cfg) => match tool_cfg.into_config(self.codex_linux_sandbox_exe.clone()) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        let result = CallToolResult {
                            content: vec![ContentBlock::TextContent(TextContent {
                                r#type: "text".to_owned(),
                                text: format!(
                                    "Failed to load Codex configuration from overrides: {e}"
                                ),
                                annotations: None,
                            })],
                            is_error: Some(true),
                            structured_content: None,
                        };
                        self.send_response::<mcp_types::CallToolRequest>(id, result)
                            .await;
                        return;
                    }
                },
                Err(e) => {
                    let result = CallToolResult {
                        content: vec![ContentBlock::TextContent(TextContent {
                            r#type: "text".to_owned(),
                            text: format!("Failed to parse configuration for Codex tool: {e}"),
                            annotations: None,
                        })],
                        is_error: Some(true),
                        structured_content: None,
                    };
                    self.send_response::<mcp_types::CallToolRequest>(id, result)
                        .await;
                    return;
                }
            },
            None => {
                let result = CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text:
                            "Missing arguments for codex tool-call; the `prompt` field is required."
                                .to_string(),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: None,
                };
                self.send_response::<mcp_types::CallToolRequest>(id, result)
                    .await;
                return;
            }
        };

        // Clone outgoing and server to move into async task.
        let outgoing = self.outgoing.clone();
        let conversation_manager = self.conversation_manager.clone();
        let running_requests_id_to_codex_uuid = self.running_requests_id_to_codex_uuid.clone();

        // Spawn an async task to handle the Codex session so that we do not
        // block the synchronous message-processing loop.
        task::spawn(async move {
            // Run the Codex session and stream events back to the client.
            crate::codex_tool_runner::run_codex_tool_session(
                id,
                initial_prompt,
                config,
                outgoing,
                conversation_manager,
                running_requests_id_to_codex_uuid,
            )
            .await;
        });
    }

    async fn handle_tool_call_codex_session_reply(
        &self,
        request_id: RequestId,
        arguments: Option<serde_json::Value>,
    ) {
        tracing::info!("tools/call -> params: {:?}", arguments);

        // parse arguments
        let CodexToolCallReplyParam {
            conversation_id,
            prompt,
        } = match arguments {
            Some(json_val) => match serde_json::from_value::<CodexToolCallReplyParam>(json_val) {
                Ok(params) => params,
                Err(e) => {
                    tracing::error!("Failed to parse Codex tool call reply parameters: {e}");
                    let result = CallToolResult {
                        content: vec![ContentBlock::TextContent(TextContent {
                            r#type: "text".to_owned(),
                            text: format!("Failed to parse configuration for Codex tool: {e}"),
                            annotations: None,
                        })],
                        is_error: Some(true),
                        structured_content: None,
                    };
                    self.send_response::<mcp_types::CallToolRequest>(request_id, result)
                        .await;
                    return;
                }
            },
            None => {
                tracing::error!(
                    "Missing arguments for codex-reply tool-call; the `conversation_id` and `prompt` fields are required."
                );
                let result = CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_owned(),
                        text: "Missing arguments for codex-reply tool-call; the `conversation_id` and `prompt` fields are required.".to_owned(),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: None,
                };
                self.send_response::<mcp_types::CallToolRequest>(request_id, result)
                    .await;
                return;
            }
        };
        let conversation_id = match ConversationId::from_string(&conversation_id) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Failed to parse conversation_id: {e}");
                let result = CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_owned(),
                        text: format!("Failed to parse conversation_id: {e}"),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: None,
                };
                self.send_response::<mcp_types::CallToolRequest>(request_id, result)
                    .await;
                return;
            }
        };

        // Clone outgoing to move into async task.
        let outgoing = self.outgoing.clone();
        let running_requests_id_to_codex_uuid = self.running_requests_id_to_codex_uuid.clone();

        let codex = match self
            .conversation_manager
            .get_conversation(conversation_id)
            .await
        {
            Ok(c) => c,
            Err(_) => {
                tracing::warn!("Session not found for conversation_id: {conversation_id}");
                let result = CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_owned(),
                        text: format!("Session not found for conversation_id: {conversation_id}"),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: None,
                };
                outgoing.send_response(request_id, result).await;
                return;
            }
        };

        // Spawn the long-running reply handler.
        tokio::spawn({
            let outgoing = outgoing.clone();
            let prompt = prompt.clone();
            let running_requests_id_to_codex_uuid = running_requests_id_to_codex_uuid.clone();

            async move {
                crate::codex_tool_runner::run_codex_tool_session_reply(
                    codex,
                    outgoing,
                    request_id,
                    prompt,
                    running_requests_id_to_codex_uuid,
                    conversation_id,
                )
                .await;
            }
        });
    }

    fn handle_set_level(
        &self,
        params: <mcp_types::SetLevelRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("logging/setLevel -> params: {:?}", params);
    }

    fn handle_complete(
        &self,
        params: <mcp_types::CompleteRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("completion/complete -> params: {:?}", params);
    }

    // ---------------------------------------------------------------------
    // Notification handlers
    // ---------------------------------------------------------------------

    async fn handle_cancelled_notification(
        &self,
        params: <mcp_types::CancelledNotification as mcp_types::ModelContextProtocolNotification>::Params,
    ) {
        let request_id = params.request_id;
        // Create a stable string form early for logging and submission id.
        let request_id_string = match &request_id {
            RequestId::String(s) => s.clone(),
            RequestId::Integer(i) => i.to_string(),
        };

        // Obtain the conversation id while holding the first lock, then release.
        let conversation_id = {
            let map_guard = self.running_requests_id_to_codex_uuid.lock().await;
            match map_guard.get(&request_id) {
                Some(id) => *id,
                None => {
                    tracing::warn!("Session not found for request_id: {}", request_id_string);
                    return;
                }
            }
        };
        tracing::info!("conversation_id: {conversation_id}");

        // Obtain the Codex conversation from the server.
        let codex_arc = match self
            .conversation_manager
            .get_conversation(conversation_id)
            .await
        {
            Ok(c) => c,
            Err(_) => {
                tracing::warn!("Session not found for conversation_id: {conversation_id}");
                return;
            }
        };

        // Submit interrupt to Codex.
        let err = codex_arc
            .submit_with_id(Submission {
                id: request_id_string,
                op: codex_core::protocol::Op::Interrupt,
            })
            .await;
        if let Err(e) = err {
            tracing::error!("Failed to submit interrupt to Codex: {e}");
            return;
        }
        // unregister the id so we don't keep it in the map
        self.running_requests_id_to_codex_uuid
            .lock()
            .await
            .remove(&request_id);
    }

    fn handle_progress_notification(
        &self,
        params: <mcp_types::ProgressNotification as mcp_types::ModelContextProtocolNotification>::Params,
    ) {
        tracing::info!("notifications/progress -> params: {:?}", params);
    }

    fn handle_resource_list_changed(
        &self,
        params: <mcp_types::ResourceListChangedNotification as mcp_types::ModelContextProtocolNotification>::Params,
    ) {
        tracing::info!(
            "notifications/resources/list_changed -> params: {:?}",
            params
        );
    }

    fn handle_resource_updated(
        &self,
        params: <mcp_types::ResourceUpdatedNotification as mcp_types::ModelContextProtocolNotification>::Params,
    ) {
        tracing::info!("notifications/resources/updated -> params: {:?}", params);
    }

    fn handle_prompt_list_changed(
        &self,
        params: <mcp_types::PromptListChangedNotification as mcp_types::ModelContextProtocolNotification>::Params,
    ) {
        tracing::info!("notifications/prompts/list_changed -> params: {:?}", params);
    }

    fn handle_tool_list_changed(
        &self,
        params: <mcp_types::ToolListChangedNotification as mcp_types::ModelContextProtocolNotification>::Params,
    ) {
        tracing::info!("notifications/tools/list_changed -> params: {:?}", params);
    }

    fn handle_logging_message(
        &self,
        params: <mcp_types::LoggingMessageNotification as mcp_types::ModelContextProtocolNotification>::Params,
    ) {
        tracing::info!("notifications/message -> params: {:?}", params);
    }
}
