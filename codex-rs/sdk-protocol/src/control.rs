//! Control protocol for bidirectional SDK-CLI communication.
//!
//! This module defines the request/response protocol used for:
//! - Permission checks (can_use_tool)
//! - Hook callbacks
//! - MCP message routing
//! - Session control (interrupt, set model, etc.)

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use ts_rs::TS;

use crate::PermissionMode;
use crate::hooks::HookEvent;
use crate::hooks::HookInput;
use crate::hooks::HookOutput;

// ============================================================================
// Handshake Messages
// ============================================================================

/// SDK hello message for version negotiation (SDK → CLI).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct SdkHello {
    /// Protocol version (e.g., "1.0").
    pub version: String,
    /// Capabilities this SDK supports.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// CLI hello response (CLI → SDK).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct CliHello {
    /// Protocol version (e.g., "1.0").
    pub version: String,
    /// Session identifier.
    pub session_id: String,
    /// Capabilities this CLI supports.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

// ============================================================================
// Control Request/Response Envelope
// ============================================================================

/// Envelope for control requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ControlRequestEnvelope {
    /// Unique request identifier for correlation.
    pub request_id: String,
    /// The actual request payload.
    pub request: ControlRequest,
}

/// Envelope for control responses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ControlResponseEnvelope {
    /// Request ID this response correlates to.
    pub request_id: String,
    /// The actual response payload.
    pub response: ControlResponse,
}

// ============================================================================
// Outbound Control Requests (SDK → CLI)
// ============================================================================

/// Control requests sent from SDK to CLI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum OutboundControlRequest {
    /// Interrupt current operation.
    Interrupt,
    /// Set permission mode.
    SetPermissionMode { mode: PermissionMode },
    /// Set model for subsequent turns.
    SetModel { model: Option<String> },
    /// Rewind files to a previous checkpoint.
    RewindFiles { user_message_id: String },
    /// Stream additional input messages.
    StreamInput { input: Vec<JsonValue> },
}

/// Response to outbound control requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum OutboundControlResponse {
    /// Success response.
    Success,
    /// Error response.
    Error { message: String },
}

// ============================================================================
// Inbound Control Requests (CLI → SDK)
// ============================================================================

/// Control requests sent from CLI to SDK.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum InboundControlRequest {
    /// Request permission decision for tool usage.
    CanUseTool(CanUseToolRequest),
    /// Execute a hook callback.
    HookCallback(HookCallbackRequest),
    /// Route an MCP message.
    McpMessage(McpMessageRequest),
}

/// Request permission decision for tool usage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct CanUseToolRequest {
    /// Name of the tool being invoked.
    pub tool_name: String,
    /// Tool input parameters.
    pub input: JsonValue,
    /// Suggested permission updates.
    #[serde(default)]
    pub permission_suggestions: Vec<PermissionSuggestion>,
    /// Path that triggered the permission check (if applicable).
    pub blocked_path: Option<String>,
    /// Formatted reason for the permission check.
    pub decision_reason: Option<String>,
    /// Unique tool use identifier.
    pub tool_use_id: String,
    /// Agent making the request.
    pub agent_id: String,
}

/// A suggested permission update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PermissionSuggestion {
    /// Allow this invocation once.
    AllowOnce,
    /// Deny this invocation once.
    DenyOnce,
    /// Allow always with a pattern.
    AllowAlways { pattern: String },
    /// Deny always with a pattern.
    DenyAlways { pattern: String },
}

/// Response to can_use_tool request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct CanUseToolResponse {
    /// The permission decision.
    pub behavior: PermissionBehavior,
    /// Optional message to display.
    pub message: Option<String>,
    /// Tool use ID this response is for.
    pub tool_use_id: Option<String>,
}

/// Permission behavior decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PermissionBehavior {
    /// Allow the tool invocation.
    Allow,
    /// Deny the tool invocation.
    Deny,
    /// Prompt the user (for interactive mode).
    Prompt,
}

/// Hook callback request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct HookCallbackRequest {
    /// SDK-assigned callback identifier.
    pub callback_id: String,
    /// Hook event type.
    pub hook_event: HookEvent,
    /// Hook input data.
    pub input: HookInput,
    /// Associated tool use ID (if applicable).
    pub tool_use_id: Option<String>,
}

/// Response to hook callback request.
pub type HookCallbackResponse = HookOutput;

/// MCP message routing request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct McpMessageRequest {
    /// MCP server name.
    pub server_name: String,
    /// MCP protocol message.
    pub message: JsonValue,
}

/// Response to MCP message request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct McpMessageResponse {
    /// MCP protocol response.
    pub mcp_response: JsonValue,
}

// ============================================================================
// Inbound Control Response (SDK → CLI)
// ============================================================================

/// Responses to inbound control requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum InboundControlResponse {
    /// Response to can_use_tool.
    CanUseToolResponse(CanUseToolResponse),
    /// Response to hook_callback.
    HookCallbackResponse(HookCallbackResponse),
    /// Response to mcp_message.
    McpMessageResponse(McpMessageResponse),
    /// Error response.
    Error { message: String },
}

// ============================================================================
// Unified Control Request/Response
// ============================================================================

/// Unified control request (both directions).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(untagged)]
pub enum ControlRequest {
    Outbound(OutboundControlRequest),
    Inbound(InboundControlRequest),
}

/// Unified control response (both directions).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(untagged)]
pub enum ControlResponse {
    Outbound(OutboundControlResponse),
    Inbound(InboundControlResponse),
}

// ============================================================================
// Cancel Request
// ============================================================================

/// Cancel a pending control request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ControlCancelRequest {
    /// The request ID to cancel.
    pub request_id: String,
}
