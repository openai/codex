use super::shared::v2_enum_from_core;
use codex_protocol::approvals::ElicitationRequest as CoreElicitationRequest;
pub use codex_protocol::approvals::McpElicitationArrayType;
pub use codex_protocol::approvals::McpElicitationBooleanSchema;
pub use codex_protocol::approvals::McpElicitationBooleanType;
pub use codex_protocol::approvals::McpElicitationConstOption;
pub use codex_protocol::approvals::McpElicitationEnumSchema;
pub use codex_protocol::approvals::McpElicitationLegacyTitledEnumSchema;
pub use codex_protocol::approvals::McpElicitationMultiSelectEnumSchema;
pub use codex_protocol::approvals::McpElicitationNumberSchema;
pub use codex_protocol::approvals::McpElicitationNumberType;
pub use codex_protocol::approvals::McpElicitationObjectType;
pub use codex_protocol::approvals::McpElicitationPrimitiveSchema;
pub use codex_protocol::approvals::McpElicitationSchema;
pub use codex_protocol::approvals::McpElicitationSingleSelectEnumSchema;
pub use codex_protocol::approvals::McpElicitationStringFormat;
pub use codex_protocol::approvals::McpElicitationStringSchema;
pub use codex_protocol::approvals::McpElicitationStringType;
pub use codex_protocol::approvals::McpElicitationTitledEnumItems;
pub use codex_protocol::approvals::McpElicitationTitledMultiSelectEnumSchema;
pub use codex_protocol::approvals::McpElicitationTitledSingleSelectEnumSchema;
pub use codex_protocol::approvals::McpElicitationUntitledEnumItems;
pub use codex_protocol::approvals::McpElicitationUntitledMultiSelectEnumSchema;
pub use codex_protocol::approvals::McpElicitationUntitledSingleSelectEnumSchema;
use codex_protocol::items::McpToolCallError as CoreMcpToolCallError;
use codex_protocol::mcp::CallToolResult as CoreMcpCallToolResult;
use codex_protocol::mcp::McpServerInfo;
use codex_protocol::mcp::Resource as McpResource;
pub use codex_protocol::mcp::ResourceContent as McpResourceContent;
use codex_protocol::mcp::ResourceTemplate as McpResourceTemplate;
use codex_protocol::mcp::Tool as McpTool;
use schemars::JsonSchema;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Deserialize;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Serialize;
use serde_json::Value as JsonValue;
use ts_rs::TS;

v2_enum_from_core!(
    pub enum McpAuthStatus from codex_protocol::protocol::McpAuthStatus {
        Unsupported,
        NotLoggedIn,
        BearerToken,
        OAuth
    }
);

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ListMcpServerStatusParams {
    /// Opaque pagination cursor returned by a previous call.
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    /// Optional page size; defaults to a server-defined value.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
    /// Controls how much MCP inventory data to fetch for each server.
    /// Defaults to `Full` when omitted.
    #[ts(optional = nullable)]
    pub detail: Option<McpServerStatusDetail>,
    #[ts(optional = nullable)]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum McpServerStatusDetail {
    Full,
    ToolsAndAuthOnly,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerStatus {
    pub name: String,
    pub server_info: Option<McpServerInfo>,
    pub tools: std::collections::HashMap<String, McpTool>,
    pub resources: Vec<McpResource>,
    pub resource_templates: Vec<McpResourceTemplate>,
    pub auth_status: McpAuthStatus,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct ListMcpServerStatusResponse {
    pub data: Vec<McpServerStatus>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// If None, there are no more items to return.
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpResourceReadParams {
    #[ts(optional = nullable)]
    pub thread_id: Option<String>,
    pub server: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpResourceReadResponse {
    pub contents: Vec<McpResourceContent>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerToolCallParams {
    pub thread_id: String,
    pub server: String,
    pub tool: String,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[ts(optional)]
    pub arguments: Option<JsonValue>,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")
    )]
    #[ts(optional)]
    pub meta: Option<JsonValue>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerToolCallResponse {
    pub content: Vec<JsonValue>,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[ts(optional)]
    pub structured_content: Option<JsonValue>,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[ts(optional)]
    pub is_error: Option<bool>,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")
    )]
    #[ts(optional)]
    pub meta: Option<JsonValue>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpToolCallResult {
    // NOTE: `rmcp::model::Content` (and its `RawContent` variants) would be a more precise Rust
    // representation of MCP content blocks. We intentionally use `serde_json::Value` here because
    // this crate exports JSON schema + TS types (`schemars`/`ts-rs`), and the rmcp model types
    // aren't set up to be schema/TS friendly (and would introduce heavier coupling to rmcp's Rust
    // representations). Using `JsonValue` keeps the payload wire-shaped and easy to export.
    pub content: Vec<JsonValue>,
    pub structured_content: Option<JsonValue>,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "_meta"))]
    #[ts(rename = "_meta")]
    pub meta: Option<JsonValue>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpToolCallError {
    pub message: String,
}

impl From<CoreMcpCallToolResult> for McpServerToolCallResponse {
    fn from(result: CoreMcpCallToolResult) -> Self {
        Self {
            content: result.content,
            structured_content: result.structured_content,
            is_error: result.is_error,
            meta: result.meta,
        }
    }
}

impl From<CoreMcpCallToolResult> for McpToolCallResult {
    fn from(result: CoreMcpCallToolResult) -> Self {
        Self {
            content: result.content,
            structured_content: result.structured_content,
            meta: result.meta,
        }
    }
}

impl From<CoreMcpToolCallError> for McpToolCallError {
    fn from(error: CoreMcpToolCallError) -> Self {
        Self {
            message: error.message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerRefreshParams {}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerRefreshResponse {}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerOauthLoginParams {
    pub name: String,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[ts(optional = nullable)]
    pub scopes: Option<Vec<String>>,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[ts(optional = nullable)]
    pub timeout_secs: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerOauthLoginResponse {
    pub authorization_url: String,
}
#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpToolCallProgressNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerOauthLoginCompletedNotification {
    pub name: String,
    pub success: bool,
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[ts(optional)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub enum McpServerStartupState {
    Starting,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerStatusUpdatedNotification {
    pub name: String,
    pub status: McpServerStartupState,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum McpServerElicitationAction {
    Accept,
    Decline,
    Cancel,
}

impl McpServerElicitationAction {
    pub fn to_core(self) -> codex_protocol::approvals::ElicitationAction {
        match self {
            Self::Accept => codex_protocol::approvals::ElicitationAction::Accept,
            Self::Decline => codex_protocol::approvals::ElicitationAction::Decline,
            Self::Cancel => codex_protocol::approvals::ElicitationAction::Cancel,
        }
    }
}

impl From<McpServerElicitationAction> for rmcp::model::ElicitationAction {
    fn from(value: McpServerElicitationAction) -> Self {
        match value {
            McpServerElicitationAction::Accept => Self::Accept,
            McpServerElicitationAction::Decline => Self::Decline,
            McpServerElicitationAction::Cancel => Self::Cancel,
        }
    }
}

impl From<rmcp::model::ElicitationAction> for McpServerElicitationAction {
    fn from(value: rmcp::model::ElicitationAction) -> Self {
        match value {
            rmcp::model::ElicitationAction::Accept => Self::Accept,
            rmcp::model::ElicitationAction::Decline => Self::Decline,
            rmcp::model::ElicitationAction::Cancel => Self::Cancel,
        }
    }
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerElicitationRequestParams {
    pub thread_id: String,
    /// Active Codex turn when this elicitation was observed, if app-server could correlate one.
    ///
    /// This is nullable because MCP models elicitation as a standalone server-to-client request
    /// identified by the MCP server request id. It may be triggered during a turn, but turn
    /// context is app-server correlation rather than part of the protocol identity of the
    /// elicitation itself.
    pub turn_id: Option<String>,
    pub server_name: String,
    #[cfg_attr(any(test, feature = "serde-compat"), serde(flatten))]
    pub request: McpServerElicitationRequest,
    // TODO: When core can correlate an elicitation with an MCP tool call, expose the associated
    // McpToolCall item id here as an optional field. The current core event does not carry that
    // association.
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(
    any(test, feature = "serde-compat"),
    serde(tag = "mode", rename_all = "camelCase")
)]
#[ts(tag = "mode")]
#[ts(export_to = "v2/")]
pub enum McpServerElicitationRequest {
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
    #[ts(rename_all = "camelCase")]
    Form {
        #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "_meta"))]
        #[ts(rename = "_meta")]
        meta: Option<JsonValue>,
        message: String,
        requested_schema: McpElicitationSchema,
    },
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
    #[ts(rename_all = "camelCase")]
    Url {
        #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "_meta"))]
        #[ts(rename = "_meta")]
        meta: Option<JsonValue>,
        message: String,
        url: String,
        elicitation_id: String,
    },
}

impl From<CoreElicitationRequest> for McpServerElicitationRequest {
    fn from(value: CoreElicitationRequest) -> Self {
        match value {
            CoreElicitationRequest::Form {
                meta,
                message,
                requested_schema,
            } => Self::Form {
                meta,
                message,
                requested_schema,
            },
            CoreElicitationRequest::Url {
                meta,
                message,
                url,
                elicitation_id,
            } => Self::Url {
                meta,
                message,
                url,
                elicitation_id,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct McpServerElicitationRequestResponse {
    pub action: McpServerElicitationAction,
    /// Structured user input for accepted elicitations, mirroring RMCP `CreateElicitationResult`.
    ///
    /// This is nullable because decline/cancel responses have no content.
    pub content: Option<JsonValue>,
    /// Optional client metadata for form-mode action handling.
    #[cfg_attr(any(test, feature = "serde-compat"), serde(rename = "_meta"))]
    #[ts(rename = "_meta")]
    pub meta: Option<JsonValue>,
}

impl From<McpServerElicitationRequestResponse> for rmcp::model::CreateElicitationResult {
    fn from(value: McpServerElicitationRequestResponse) -> Self {
        Self {
            action: value.action.into(),
            content: value.content,
            meta: None,
        }
    }
}

impl From<rmcp::model::CreateElicitationResult> for McpServerElicitationRequestResponse {
    fn from(value: rmcp::model::CreateElicitationResult) -> Self {
        Self {
            action: value.action.into(),
            content: value.content,
            meta: None,
        }
    }
}
