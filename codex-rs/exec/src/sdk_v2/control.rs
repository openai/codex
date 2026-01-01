//! Control protocol handler for SDK ↔ CLI communication.
//!
//! Provides high-level methods for sending control requests to the SDK
//! and handling responses.

use std::time::Duration;

use codex_sdk_protocol::control::CanUseToolRequest;
use codex_sdk_protocol::control::CanUseToolResponse;
use codex_sdk_protocol::control::ControlRequest;
use codex_sdk_protocol::control::ControlResponse;
use codex_sdk_protocol::control::HookCallbackRequest;
use codex_sdk_protocol::control::InboundControlRequest;
use codex_sdk_protocol::control::InboundControlResponse;
use codex_sdk_protocol::control::McpMessageRequest;
use codex_sdk_protocol::control::McpMessageResponse;
use codex_sdk_protocol::control::PermissionBehavior;
use codex_sdk_protocol::control::PermissionSuggestion;
use codex_sdk_protocol::hooks::HookEvent;
use codex_sdk_protocol::hooks::HookInput;
use codex_sdk_protocol::hooks::HookOutput;
use serde_json::Value as JsonValue;

use super::transport::SdkTransport;
use super::transport::TransportError;

/// Default timeout for control requests.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Control protocol handler.
pub struct ControlProtocol {
    transport: SdkTransport,
    request_timeout: Duration,
}

impl ControlProtocol {
    /// Create a new control protocol handler.
    pub fn new(transport: SdkTransport) -> Self {
        Self {
            transport,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }

    /// Create with custom timeout.
    pub fn with_timeout(transport: SdkTransport, timeout: Duration) -> Self {
        Self {
            transport,
            request_timeout: timeout,
        }
    }

    /// Get mutable reference to the underlying transport.
    pub fn transport_mut(&mut self) -> &mut SdkTransport {
        &mut self.transport
    }

    /// Get the request timeout.
    pub fn request_timeout(&self) -> Duration {
        self.request_timeout
    }

    /// Request permission decision for tool usage.
    ///
    /// Sends a `can_use_tool` request to the SDK and waits for the decision.
    pub async fn can_use_tool(
        &mut self,
        tool_name: &str,
        input: &JsonValue,
        suggestions: Vec<PermissionSuggestion>,
        tool_use_id: &str,
        agent_id: &str,
    ) -> Result<CanUseToolResponse, ControlError> {
        let request = ControlRequest::Inbound(InboundControlRequest::CanUseTool(
            CanUseToolRequest {
                tool_name: tool_name.to_string(),
                input: input.clone(),
                permission_suggestions: suggestions,
                blocked_path: None,
                decision_reason: None,
                tool_use_id: tool_use_id.to_string(),
                agent_id: agent_id.to_string(),
            },
        ));

        let response = self.send_request_with_timeout(request).await?;

        match response {
            ControlResponse::Inbound(InboundControlResponse::CanUseToolResponse(resp)) => Ok(resp),
            ControlResponse::Inbound(InboundControlResponse::Error { message }) => {
                Err(ControlError::RemoteError(message))
            }
            _ => Err(ControlError::UnexpectedResponse),
        }
    }

    /// Execute a hook callback.
    ///
    /// Sends a hook callback request to the SDK and waits for the result.
    pub async fn hook_callback(
        &mut self,
        callback_id: &str,
        hook_event: HookEvent,
        input: HookInput,
        tool_use_id: Option<&str>,
    ) -> Result<HookOutput, ControlError> {
        let request = ControlRequest::Inbound(InboundControlRequest::HookCallback(
            HookCallbackRequest {
                callback_id: callback_id.to_string(),
                hook_event,
                input,
                tool_use_id: tool_use_id.map(|s| s.to_string()),
            },
        ));

        let response = self.send_request_with_timeout(request).await?;

        match response {
            ControlResponse::Inbound(InboundControlResponse::HookCallbackResponse(output)) => {
                Ok(output)
            }
            ControlResponse::Inbound(InboundControlResponse::Error { message }) => {
                Err(ControlError::RemoteError(message))
            }
            _ => Err(ControlError::UnexpectedResponse),
        }
    }

    /// Route an MCP message to the SDK.
    ///
    /// Used for SDK-hosted MCP servers.
    pub async fn mcp_message(
        &mut self,
        server_name: &str,
        message: JsonValue,
    ) -> Result<McpMessageResponse, ControlError> {
        let request = ControlRequest::Inbound(InboundControlRequest::McpMessage(
            McpMessageRequest {
                server_name: server_name.to_string(),
                message,
            },
        ));

        let response = self.send_request_with_timeout(request).await?;

        match response {
            ControlResponse::Inbound(InboundControlResponse::McpMessageResponse(resp)) => Ok(resp),
            ControlResponse::Inbound(InboundControlResponse::Error { message }) => {
                Err(ControlError::RemoteError(message))
            }
            _ => Err(ControlError::UnexpectedResponse),
        }
    }

    /// Send a control request with timeout.
    async fn send_request_with_timeout(
        &mut self,
        request: ControlRequest,
    ) -> Result<ControlResponse, ControlError> {
        tokio::time::timeout(self.request_timeout, self.transport.send_control_request(request))
            .await
            .map_err(|_| ControlError::Timeout)?
            .map_err(ControlError::Transport)
    }
}

/// Error type for control protocol operations.
#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    #[error("Request timeout")]
    Timeout,

    #[error("Remote error: {0}")]
    RemoteError(String),

    #[error("Unexpected response type")]
    UnexpectedResponse,
}

/// Helper functions for working with PermissionBehavior.
pub mod permission_behavior_ext {
    use codex_sdk_protocol::control::PermissionBehavior;

    /// Check if this behavior allows the operation.
    pub fn is_allowed(behavior: &PermissionBehavior) -> bool {
        matches!(behavior, PermissionBehavior::Allow)
    }

    /// Check if this behavior denies the operation.
    pub fn is_denied(behavior: &PermissionBehavior) -> bool {
        matches!(behavior, PermissionBehavior::Deny)
    }

    /// Check if this behavior requires prompting the user.
    pub fn requires_prompt(behavior: &PermissionBehavior) -> bool {
        matches!(behavior, PermissionBehavior::Prompt)
    }
}
