use crate::codex_tool_config::CodexToolCallParam;
use crate::codex_tool_config::create_tool_for_codex_tool_call_param;

use codex_core::config::Config as CodexConfig;
use mcp_types::CallToolRequestParams;
use mcp_types::CallToolResult;
use mcp_types::CallToolResultContent;
use mcp_types::ClientRequest;
use mcp_types::JSONRPC_VERSION;
use mcp_types::JSONRPCBatchRequest;
use mcp_types::JSONRPCBatchResponse;
use mcp_types::JSONRPCError;
use mcp_types::JSONRPCErrorError;
use mcp_types::JSONRPCMessage;
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
use tokio::sync::mpsc;
use tokio::task;

pub(crate) struct MessageProcessor {
    outgoing: mpsc::Sender<JSONRPCMessage>,
    initialized: bool,
}

impl MessageProcessor {
    /// Create a new `MessageProcessor`, retaining a handle to the outgoing
    /// `Sender` so handlers can enqueue messages to be written to stdout.
    pub(crate) fn new(outgoing: mpsc::Sender<JSONRPCMessage>) -> Self {
        Self {
            outgoing,
            initialized: false,
        }
    }

    pub(crate) fn process_request(&mut self, request: JSONRPCRequest) {
        // Hold on to the ID so we can respond.
        let request_id = request.id.clone();

        let client_request = match ClientRequest::try_from(request) {
            Ok(client_request) => client_request,
            Err(e) => {
                tracing::warn!("Failed to convert request: {e}");
                return;
            }
        };

        // Dispatch to a dedicated handler for each request type.
        match client_request {
            ClientRequest::InitializeRequest(params) => {
                self.handle_initialize(request_id, params);
            }
            ClientRequest::PingRequest(params) => {
                self.handle_ping(request_id, params);
            }
            ClientRequest::ListResourcesRequest(params) => {
                self.handle_list_resources(params);
            }
            ClientRequest::ListResourceTemplatesRequest(params) => {
                self.handle_list_resource_templates(params);
            }
            ClientRequest::ReadResourceRequest(params) => {
                self.handle_read_resource(params);
            }
            ClientRequest::SubscribeRequest(params) => {
                self.handle_subscribe(params);
            }
            ClientRequest::UnsubscribeRequest(params) => {
                self.handle_unsubscribe(params);
            }
            ClientRequest::ListPromptsRequest(params) => {
                self.handle_list_prompts(params);
            }
            ClientRequest::GetPromptRequest(params) => {
                self.handle_get_prompt(params);
            }
            ClientRequest::ListToolsRequest(params) => {
                self.handle_list_tools(request_id, params);
            }
            ClientRequest::CallToolRequest(params) => {
                self.handle_call_tool(request_id, params);
            }
            ClientRequest::SetLevelRequest(params) => {
                self.handle_set_level(params);
            }
            ClientRequest::CompleteRequest(params) => {
                self.handle_complete(params);
            }
        }
    }

    /// Handle a standalone JSON-RPC response originating from the peer.
    pub(crate) fn process_response(&mut self, response: JSONRPCResponse) {
        tracing::info!("<- response: {:?}", response);
    }

    /// Handle a fire-and-forget JSON-RPC notification.
    pub(crate) fn process_notification(&mut self, notification: JSONRPCNotification) {
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
                self.handle_cancelled_notification(params);
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

    /// Handle a batch of requests and/or notifications.
    pub(crate) fn process_batch_request(&mut self, batch: JSONRPCBatchRequest) {
        tracing::info!("<- batch request containing {} item(s)", batch.len());
        for item in batch {
            match item {
                mcp_types::JSONRPCBatchRequestItem::JSONRPCRequest(req) => {
                    self.process_request(req);
                }
                mcp_types::JSONRPCBatchRequestItem::JSONRPCNotification(note) => {
                    self.process_notification(note);
                }
            }
        }
    }

    /// Handle an error object received from the peer.
    pub(crate) fn process_error(&mut self, err: JSONRPCError) {
        tracing::error!("<- error: {:?}", err);
    }

    /// Handle a batch of responses/errors.
    pub(crate) fn process_batch_response(&mut self, batch: JSONRPCBatchResponse) {
        tracing::info!("<- batch response containing {} item(s)", batch.len());
        for item in batch {
            match item {
                mcp_types::JSONRPCBatchResponseItem::JSONRPCResponse(resp) => {
                    self.process_response(resp);
                }
                mcp_types::JSONRPCBatchResponseItem::JSONRPCError(err) => {
                    self.process_error(err);
                }
            }
        }
    }

    fn handle_initialize(
        &mut self,
        id: RequestId,
        params: <mcp_types::InitializeRequest as ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("initialize -> params: {:?}", params);

        if self.initialized {
            // Already initialised: send JSON-RPC error response.
            let error_msg = JSONRPCMessage::Error(JSONRPCError {
                jsonrpc: JSONRPC_VERSION.into(),
                id,
                error: JSONRPCErrorError {
                    code: -32600, // Invalid Request
                    message: "initialize called more than once".to_string(),
                    data: None,
                },
            });

            if let Err(e) = self.outgoing.try_send(error_msg) {
                tracing::error!("Failed to send initialization error: {e}");
            }
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
                version: mcp_types::MCP_SCHEMA_VERSION.to_string(),
            },
        };

        self.send_response::<mcp_types::InitializeRequest>(id, result);
    }

    fn send_response<T>(&self, id: RequestId, result: T::Result)
    where
        T: ModelContextProtocolRequest,
    {
        let response = JSONRPCMessage::Response(JSONRPCResponse {
            jsonrpc: JSONRPC_VERSION.into(),
            id,
            result: serde_json::to_value(result).unwrap(),
        });

        if let Err(e) = self.outgoing.try_send(response) {
            tracing::error!("Failed to send response: {e}");
        }
    }

    fn handle_ping(
        &self,
        id: RequestId,
        params: <mcp_types::PingRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("ping -> params: {:?}", params);
        let result = json!({});
        self.send_response::<mcp_types::PingRequest>(id, result);
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

    fn handle_list_tools(
        &self,
        id: RequestId,
        params: <mcp_types::ListToolsRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::trace!("tools/list -> {params:?}");
        let result = ListToolsResult {
            tools: vec![create_tool_for_codex_tool_call_param()],
            next_cursor: None,
        };

        self.send_response::<mcp_types::ListToolsRequest>(id, result);
    }

    fn handle_call_tool(
        &self,
        id: RequestId,
        params: <mcp_types::CallToolRequest as mcp_types::ModelContextProtocolRequest>::Params,
    ) {
        tracing::info!("tools/call -> params: {:?}", params);
        let CallToolRequestParams { name, arguments } = params;

        // We only support the "codex" tool for now.
        if name != "codex" {
            // Tool not found – return error result so the LLM can react.
            let result = CallToolResult {
                content: vec![CallToolResultContent::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Unknown tool '{name}'"),
                    annotations: None,
                })],
                is_error: Some(true),
            };
            self.send_response::<mcp_types::CallToolRequest>(id, result);
            return;
        }

        let (initial_prompt, config): (String, CodexConfig) = match arguments {
            Some(json_val) => match serde_json::from_value::<CodexToolCallParam>(json_val) {
                Ok(tool_cfg) => match tool_cfg.into_config() {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        let result = CallToolResult {
                            content: vec![CallToolResultContent::TextContent(TextContent {
                                r#type: "text".to_owned(),
                                text: format!(
                                    "Failed to load Codex configuration from overrides: {e}"
                                ),
                                annotations: None,
                            })],
                            is_error: Some(true),
                        };
                        self.send_response::<mcp_types::CallToolRequest>(id, result);
                        return;
                    }
                },
                Err(e) => {
                    let result = CallToolResult {
                        content: vec![CallToolResultContent::TextContent(TextContent {
                            r#type: "text".to_owned(),
                            text: format!("Failed to parse configuration for Codex tool: {e}"),
                            annotations: None,
                        })],
                        is_error: Some(true),
                    };
                    self.send_response::<mcp_types::CallToolRequest>(id, result);
                    return;
                }
            },
            None => {
                let result = CallToolResult {
                    content: vec![CallToolResultContent::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text:
                            "Missing arguments for codex tool-call; the `prompt` field is required."
                                .to_string(),
                        annotations: None,
                    })],
                    is_error: Some(true),
                };
                self.send_response::<mcp_types::CallToolRequest>(id, result);
                return;
            }
        };

        // Clone outgoing sender to move into async task.
        let outgoing = self.outgoing.clone();

        // Spawn an async task to handle the Codex session so that we do not
        // block the synchronous message-processing loop.
        task::spawn(async move {
            // Run the Codex session and stream events back to the client.
            crate::codex_tool_runner::run_codex_tool_session(id, initial_prompt, config, outgoing)
                .await;
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

    fn handle_cancelled_notification(
        &self,
        params: <mcp_types::CancelledNotification as mcp_types::ModelContextProtocolNotification>::Params,
    ) {
        tracing::info!("notifications/cancelled -> params: {:?}", params);
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
