use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Default timeout for codex-get-response tool (10 minutes)
/// GPT-5 research tasks can take 5+ minutes to complete thorough analysis,
/// so 600s provides adequate buffer while still catching truly stuck sessions.
pub const DEFAULT_GET_RESPONSE_TIMEOUT_SECS: u64 = 600;

use crate::codex_message_processor::CodexMessageProcessor;
use crate::session_storage::{SessionResponse, SessionResponseStorage, SessionStatus};
use crate::codex_tool_config::CodexToolCallParam;
use crate::codex_tool_config::CodexToolCallReplyParam;
use crate::codex_tool_config::CodexToolCallGetResponseParam;
use crate::codex_tool_config::create_tool_for_codex_tool_call_param;
use crate::codex_tool_config::create_tool_for_codex_tool_call_reply_param;
use crate::codex_tool_config::create_tool_for_codex_tool_call_get_response_param;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use crate::outgoing_message::OutgoingMessageSender;
use codex_protocol::mcp_protocol::ClientRequest;

use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::protocol::Submission;
use codex_core::protocol::EventMsg;
use codex_login::AuthManager;
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
use uuid::Uuid;


pub(crate) struct MessageProcessor {
    codex_message_processor: CodexMessageProcessor,
    outgoing: Arc<OutgoingMessageSender>,
    initialized: bool,
    codex_linux_sandbox_exe: Option<PathBuf>,
    conversation_manager: Arc<ConversationManager>,
    running_requests_id_to_codex_uuid: Arc<Mutex<HashMap<RequestId, Uuid>>>,
    config: Arc<Config>,
    /// Storage for completed responses in compatibility mode
    session_responses: Arc<Mutex<SessionResponseStorage>>,
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
        let auth_manager =
            AuthManager::shared(config.codex_home.clone(), config.preferred_auth_method);
        let conversation_manager = Arc::new(ConversationManager::new(auth_manager.clone()));
        let codex_message_processor = CodexMessageProcessor::new(
            auth_manager,
            conversation_manager.clone(),
            outgoing.clone(),
            codex_linux_sandbox_exe.clone(),
            config.clone(),
        );
        Self {
            codex_message_processor,
            outgoing,
            initialized: false,
            codex_linux_sandbox_exe,
            conversation_manager,
            running_requests_id_to_codex_uuid: Arc::new(Mutex::new(HashMap::new())),
            config,
            session_responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) async fn process_request(&mut self, request: JSONRPCRequest) {
        if let Ok(request_json) = serde_json::to_value(request.clone())
            && let Ok(codex_request) = serde_json::from_value::<ClientRequest>(request_json)
        {
            // If the request is a Codex request, handle it with the Codex
            // message processor.
            self.codex_message_processor
                .process_request(codex_request)
                .await;
            return;
        }

        // Hold on to the ID so we can respond.
        let request_id = request.id.clone();

        let client_request = match McpClientRequest::try_from(request) {
            Ok(client_request) => client_request,
            Err(e) => {
                tracing::warn!("failed to convert request: {e}");
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
                tracing::warn!("failed to convert notification: {e}");
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
                create_tool_for_codex_tool_call_get_response_param(),
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
            "codex-get-response" => {
                self.handle_tool_call_codex_get_response(id, arguments)
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
                Ok(tool_cfg) => match tool_cfg
                    .into_config(self.codex_linux_sandbox_exe.clone(), Some(&self.config))
                {
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

        // Compatibility mode for MCP clients that cannot handle async notifications (like Claude Code).
        // When enabled, we return an immediate response with a session ID, then process the request
        // in the background, avoiding the async notification flow that some clients can't handle.
        if config.mcp.compatibility_mode {
            // Return immediate response with session ID, process in background
            let conversation_result = self
                .conversation_manager
                .new_conversation(config.clone())
                .await;

            match conversation_result {
                Ok(new_conv) => {
                    let session_id = new_conv.conversation_id;
                    let result = CallToolResult {
                        content: vec![ContentBlock::TextContent(TextContent {
                            r#type: "text".to_string(),
                            text: format!("Session started with ID: {session_id}"),
                            annotations: None,
                        })],
                        is_error: None,
                        structured_content: Some(json!({
                            "sessionId": session_id.to_string()
                        })),
                    };
                    self.send_response::<mcp_types::CallToolRequest>(id.clone(), result)
                        .await;

                    // Store the conversation for later use with codex-reply
                    self.running_requests_id_to_codex_uuid
                        .lock()
                        .await
                        .insert(id, session_id);

                    // Initialize session response as running
                    self.session_responses.lock().await.insert(
                        session_id,
                        SessionResponse::new_running(),
                    );

                    // Now submit the initial prompt and capture responses in the background
                    let conversation = new_conv.conversation;
                    let session_responses = self.session_responses.clone();
                    task::spawn(async move {
                        // Submit the prompt
                        if let Err(e) = conversation
                            .submit(Op::UserInput {
                                items: vec![InputItem::Text {
                                    text: initial_prompt,
                                }],
                            })
                            .await
                        {
                            tracing::error!(
                                "Failed to submit initial prompt in compatibility mode: {e}"
                            );
                            // Mark as failed
                            let mut responses = session_responses.lock().await;
                            if let Some(response) = responses.remove(&session_id) {
                                responses.insert(
                                    session_id, 
                                    response.fail_with_error(
                                        format!("failed to submit prompt: {e}"), 
                                        String::new()
                                    )
                                );
                            }
                            return;
                        }

                        // Capture events and accumulate content
                        let mut content = String::new();
                        loop {
                            match conversation.next_event().await {
                                Ok(event) => {
                                    match event.msg {
                                        EventMsg::AgentMessage(msg) => {
                                            content.push_str(&msg.message);
                                        }
                                        EventMsg::AgentMessageDelta(delta) => {
                                            content.push_str(&delta.delta);
                                        }
                                        EventMsg::TaskComplete(_) => {
                                            // Mark as completed
                                            let mut responses = session_responses.lock().await;
                                            if let Some(response) = responses.remove(&session_id) {
                                                responses.insert(
                                                    session_id,
                                                    response.complete_with_content(content)
                                                );
                                            }
                                            break;
                                        }
                                        EventMsg::Error(error) => {
                                            // Mark as failed
                                            let mut responses = session_responses.lock().await;
                                            if let Some(response) = responses.remove(&session_id) {
                                                responses.insert(
                                                    session_id,
                                                    response.fail_with_error(error.message, content)
                                                );
                                            }
                                            break;
                                        }
                                        _ => {
                                            // Ignore other events
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("error reading event in compatibility mode: {e}");
                                    // Mark as failed
                                    let mut responses = session_responses.lock().await;
                                    if let Some(response) = responses.remove(&session_id) {
                                        responses.insert(
                                            session_id,
                                            response.fail_with_error(format!("event stream error: {e}"), content)
                                        );
                                    }
                                    break;
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    let result = CallToolResult {
                        content: vec![ContentBlock::TextContent(TextContent {
                            r#type: "text".to_string(),
                            text: format!("Failed to start Codex session: {e}"),
                            annotations: None,
                        })],
                        is_error: Some(true),
                        structured_content: None,
                    };
                    self.send_response::<mcp_types::CallToolRequest>(id, result)
                        .await;
                }
            }
        } else {
            // Original async mode
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
    }

    async fn handle_tool_call_codex_session_reply(
        &self,
        request_id: RequestId,
        arguments: Option<serde_json::Value>,
    ) {
        tracing::info!("tools/call -> params: {:?}", arguments);

        // parse arguments
        let CodexToolCallReplyParam { session_id, prompt } = match arguments {
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
                    "Missing arguments for codex-reply tool-call; the `session_id` and `prompt` fields are required."
                );
                let result = CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_owned(),
                        text: "Missing arguments for codex-reply tool-call; the `session_id` and `prompt` fields are required.".to_owned(),
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
        let session_id = match Uuid::parse_str(&session_id) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Failed to parse session_id: {e}");
                let result = CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_owned(),
                        text: format!("Failed to parse session_id: {e}"),
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

        // Check if we're in compatibility mode
        if self.config.mcp.compatibility_mode {
            // In compatibility mode, send immediate response
            let result = CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Continuing session {session_id}"),
                    annotations: None,
                })],
                is_error: None,
                structured_content: None,
            };
            self.send_response::<mcp_types::CallToolRequest>(request_id.clone(), result)
                .await;

            // Store the mapping for tracking
            self.running_requests_id_to_codex_uuid
                .lock()
                .await
                .insert(request_id, session_id);

            // Submit the prompt to the conversation in the background
            let codex = match self.conversation_manager.get_conversation(session_id).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(
                        "Failed to get conversation for session {}: {}",
                        session_id,
                        e
                    );
                    return;
                }
            };

            // Update session status to running
            self.session_responses.lock().await.insert(
                session_id,
                SessionResponse::new_running(),
            );

            let session_responses = self.session_responses.clone();
            task::spawn(async move {
                // Submit the prompt
                if let Err(e) = codex
                    .submit(Op::UserInput {
                        items: vec![InputItem::Text { text: prompt }],
                    })
                    .await
                {
                    tracing::error!("Failed to submit prompt in compatibility mode: {e}");
                    // Mark as failed
                    let mut responses = session_responses.lock().await;
                    if let Some(response) = responses.remove(&session_id) {
                        responses.insert(
                            session_id,
                            response.fail_with_error(format!("failed to submit prompt: {e}"), String::new())
                        );
                    }
                    return;
                }

                // Capture events and accumulate content
                let mut content = String::new();
                loop {
                    match codex.next_event().await {
                        Ok(event) => {
                            match event.msg {
                                EventMsg::AgentMessage(msg) => {
                                    content.push_str(&msg.message);
                                }
                                EventMsg::AgentMessageDelta(delta) => {
                                    content.push_str(&delta.delta);
                                }
                                EventMsg::TaskComplete(_) => {
                                    // Mark as completed
                                    let mut responses = session_responses.lock().await;
                                    if let Some(response) = responses.remove(&session_id) {
                                        responses.insert(
                                            session_id,
                                            response.complete_with_content(content)
                                        );
                                    }
                                    break;
                                }
                                EventMsg::Error(error) => {
                                    // Mark as failed
                                    let mut responses = session_responses.lock().await;
                                    if let Some(response) = responses.remove(&session_id) {
                                        responses.insert(
                                            session_id,
                                            response.fail_with_error(error.message, content)
                                        );
                                    }
                                    break;
                                }
                                _ => {
                                    // Ignore other events
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error reading event in compatibility mode: {e}");
                            // Mark as failed
                            let mut responses = session_responses.lock().await;
                            if let Some(response) = responses.remove(&session_id) {
                                responses.insert(
                                    session_id,
                                    response.fail_with_error(format!("event stream error: {e}"), content)
                                );
                            }
                            break;
                        }
                    }
                }
            });
        } else {
            // Original async mode
            // Clone outgoing to move into async task.
            let outgoing = self.outgoing.clone();
            let running_requests_id_to_codex_uuid = self.running_requests_id_to_codex_uuid.clone();

            let codex = match self.conversation_manager.get_conversation(session_id).await {
                Ok(c) => c,
                Err(_) => {
                    tracing::warn!("Session not found for session_id: {session_id}");
                    let result = CallToolResult {
                        content: vec![ContentBlock::TextContent(TextContent {
                            r#type: "text".to_owned(),
                            text: format!("Session not found for session_id: {session_id}"),
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
                let codex = codex.clone();
                let outgoing = outgoing.clone();
                let prompt = prompt.clone();
                let running_requests_id_to_codex_uuid = running_requests_id_to_codex_uuid.clone();
                let config = self.config.clone();

                async move {
                    crate::codex_tool_runner::run_codex_tool_session_reply(
                        codex,
                        outgoing,
                        request_id,
                        prompt,
                        running_requests_id_to_codex_uuid,
                        session_id,
                        (*config).clone(),
                    )
                    .await;
                }
            });
        }
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

    /// Handle the codex-get-response tool call.
    async fn handle_tool_call_codex_get_response(
        &self,
        id: RequestId,
        arguments: Option<serde_json::Value>,
    ) {
        let params = match arguments {
            Some(json_val) => match serde_json::from_value::<CodexToolCallGetResponseParam>(json_val) {
                Ok(params) => params,
                Err(e) => {
                    tracing::error!("failed to parse codex get-response parameters: {e}");
                    let result = CallToolResult {
                        content: vec![ContentBlock::TextContent(TextContent {
                            r#type: "text".to_string(),
                            text: format!("failed to parse parameters: {e}"),
                            annotations: None,
                        })],
                        is_error: Some(true),
                        structured_content: None,
                    };
                    self.send_response::<mcp_types::CallToolRequest>(id, result).await;
                    return;
                }
            },
            None => {
                let result = CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: "missing session_id parameter".to_string(),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: None,
                };
                self.send_response::<mcp_types::CallToolRequest>(id, result).await;
                return;
            }
        };

        let session_uuid = match params.session_id.parse::<Uuid>() {
            Ok(uuid) => uuid,
            Err(e) => {
                let result = CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: format!("invalid session_id format: {e}"),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: None,
                };
                self.send_response::<mcp_types::CallToolRequest>(id, result).await;
                return;
            }
        };

        let timeout_duration = Duration::from_secs(params.timeout.unwrap_or(DEFAULT_GET_RESPONSE_TIMEOUT_SECS));
        let start_time = std::time::Instant::now();

        // Poll for response with timeout
        loop {
            if start_time.elapsed() >= timeout_duration {
                let result = CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: "timeout waiting for response".to_string(),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: Some(json!({
                        "status": "timeout",
                        "sessionId": params.session_id
                    })),
                };
                self.send_response::<mcp_types::CallToolRequest>(id, result).await;
                return;
            }

            let responses = self.session_responses.lock().await;
            if let Some(response) = responses.get(&session_uuid) {
                match &response.status {
                        SessionStatus::Completed => {
                            let result = CallToolResult {
                                content: vec![ContentBlock::TextContent(TextContent {
                                    r#type: "text".to_string(),
                                    text: response.content.clone(),
                                    annotations: None,
                                })],
                                is_error: None,
                                structured_content: Some(json!({
                                    "status": "completed",
                                    "sessionId": params.session_id
                                })),
                            };
                            self.send_response::<mcp_types::CallToolRequest>(id, result).await;
                            return;
                        }
                        SessionStatus::Failed => {
                            let error_msg = response.error.as_ref().map(|s| s.as_str()).unwrap_or("unknown error");
                            let result = CallToolResult {
                                content: vec![ContentBlock::TextContent(TextContent {
                                    r#type: "text".to_string(),
                                    text: format!("session failed: {}", error_msg),
                                    annotations: None,
                                })],
                                is_error: Some(true),
                                structured_content: Some(json!({
                                    "status": "failed",
                                    "error": error_msg,
                                    "sessionId": params.session_id,
                                    "partialContent": response.content
                                })),
                            };
                            self.send_response::<mcp_types::CallToolRequest>(id, result).await;
                            return;
                        }
                        SessionStatus::Running => {
                            // Continue polling
                        }
                    }
                } else {
                    let result = CallToolResult {
                        content: vec![ContentBlock::TextContent(TextContent {
                            r#type: "text".to_string(),
                            text: "session not found".to_string(),
                            annotations: None,
                        })],
                        is_error: Some(true),
                        structured_content: Some(json!({
                            "status": "not_found",
                            "sessionId": params.session_id
                        })),
                    };
                    self.send_response::<mcp_types::CallToolRequest>(id, result).await;
                    return;
                }

            // Wait a bit before polling again
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
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

        // Obtain the session_id while holding the first lock, then release.
        let session_id = {
            let map_guard = self.running_requests_id_to_codex_uuid.lock().await;
            match map_guard.get(&request_id) {
                Some(id) => *id, // Uuid is Copy
                None => {
                    tracing::warn!("Session not found for request_id: {}", request_id_string);
                    return;
                }
            }
        };
        tracing::info!("session_id: {session_id}");

        // Obtain the Codex conversation from the server.
        let codex_arc = match self.conversation_manager.get_conversation(session_id).await {
            Ok(c) => c,
            Err(_) => {
                tracing::warn!("Session not found for session_id: {session_id}");
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
