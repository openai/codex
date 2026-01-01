//! Message types for SDK communication.
//!
//! These types define the structure of messages exchanged between SDK and CLI.

use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use ts_rs::TS;

use crate::control::CliHello;
use crate::control::ControlCancelRequest;
use crate::control::ControlRequestEnvelope;
use crate::control::ControlResponseEnvelope;
use crate::control::SdkHello;
use crate::events::ThreadEvent;

// ============================================================================
// Input Messages (SDK → CLI)
// ============================================================================

/// Messages that can be sent from SDK to CLI via stdin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SdkMessage {
    /// SDK hello for version negotiation.
    SdkHello(SdkHello),
    /// User message/prompt.
    User(UserMessage),
    /// Control request.
    ControlRequest(ControlRequestEnvelope),
    /// Control response (answering CLI's request).
    ControlResponse(ControlResponseEnvelope),
    /// Cancel a pending control request.
    ControlCancelRequest(ControlCancelRequest),
}

/// User message sent to the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct UserMessage {
    /// Session ID for this message.
    pub session_id: Option<String>,
    /// The message content.
    pub message: MessageContent,
}

/// Message content structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct MessageContent {
    /// Role of the message sender.
    pub role: MessageRole,
    /// Content of the message.
    pub content: MessageContentBody,
}

/// Role of a message sender.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Message content body.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(untagged)]
pub enum MessageContentBody {
    /// Simple text content.
    Text(String),
    /// Structured content blocks.
    Blocks(Vec<ContentBlock>),
}

/// A content block in a message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content.
    Text { text: String },
    /// Image content.
    Image {
        source: ImageSource,
        #[serde(default)]
        detail: Option<ImageDetail>,
    },
    /// File content.
    File { path: String },
    /// Tool use request.
    ToolUse {
        id: String,
        name: String,
        input: JsonValue,
    },
    /// Tool result.
    ToolResult {
        tool_use_id: String,
        content: JsonValue,
        is_error: Option<bool>,
    },
}

/// Image source for image content blocks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// Base64 encoded image data.
    Base64 { media_type: String, data: String },
    /// URL reference.
    Url { url: String },
}

/// Image detail level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ImageDetail {
    Auto,
    Low,
    High,
}

// ============================================================================
// Output Messages (CLI → SDK)
// ============================================================================

/// Messages that can be sent from CLI to SDK via stdout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliMessage {
    /// CLI hello for version negotiation.
    CliHello(CliHello),
    /// Stream event.
    StreamEvent(StreamEventMessage),
    /// Result message.
    Result(ResultMessage),
    /// Control request (CLI asking SDK).
    ControlRequest(ControlRequestEnvelope),
    /// Control response (answering SDK's request).
    ControlResponse(ControlResponseEnvelope),
}

/// Stream event wrapper.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct StreamEventMessage {
    /// Session ID.
    pub session_id: String,
    /// Unique event ID.
    pub uuid: String,
    /// The thread event.
    pub event: ThreadEvent,
}

/// Result message indicating completion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct ResultMessage {
    /// Session ID.
    pub session_id: String,
    /// Result subtype.
    pub subtype: ResultSubtype,
    /// Whether the result indicates an error.
    pub is_error: bool,
    /// Final response text (if any).
    pub response: Option<String>,
    /// Error message (if any).
    pub error: Option<String>,
    /// Thread ID for resumption.
    pub thread_id: Option<String>,
}

/// Result subtypes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResultSubtype {
    /// Successful completion.
    Success,
    /// Error occurred.
    Error,
    /// Interrupted by user/SDK.
    Interrupted,
    /// Max turns reached.
    MaxTurns,
}
